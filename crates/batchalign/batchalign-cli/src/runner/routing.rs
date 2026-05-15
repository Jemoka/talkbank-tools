//! Command dispatch routing — decides which dispatch family to invoke for a
//! given job, resolves runtime worker capabilities, and delegates to the
//! per-command dispatch wrappers.
//!
//! The central function is `dispatch_job_with_execution_context`, which is
//! called by `ExecutionEngine::dispatch_job` after all host-level concerns
//! (memory reservation, preflight, pre-scaling) have been handled.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use tracing::{info, warn};

use crate::api::{EngineVersion, NumWorkers, ReleasedCommand, RevAiJobId};
use crate::cache::UtteranceCache;

use crate::capability::resolve_worker_capability_snapshot;
use crate::commands::{RunnerDispatchKind, command_runner_dispatch_kind};
use crate::execution::{
    MorphotagRuntimeOptions, PooledWorkerGateway, WorkerGateway, dispatch_compare_job,
    dispatch_coref_job, dispatch_morphotag_job, dispatch_translate_job, dispatch_utseg_job,
};
use crate::pipeline::PipelineServices;
use crate::store::{RunnerJobSnapshot, unix_now};
use crate::worker::InferTask;
use crate::worker::pool::WorkerPool;
use crate::worker::target::task_name as infer_task_name;

use super::context::{DispatchHostContext, JobDispatchRequest, RunnerExecutionContext};
use super::dispatch::{
    BatchedInferDispatchPlan, BenchmarkDispatchPlan, BenchmarkDispatchRuntime, FaDispatchPlan,
    FaDispatchRuntime, MediaAnalysisDispatchPlan, MediaAnalysisDispatchRuntime,
    TranscribeDispatchPlan, TranscribeDispatchRuntime, dispatch_batched_infer,
    dispatch_benchmark_infer, dispatch_fa_infer, dispatch_media_analysis_v2,
    dispatch_transcribe_infer,
};
use super::policy::{command_requires_chat_infer, infer_task_for_command};
use super::test_echo::dispatch_test_echo_files;

/// Core dispatch router: resolves capabilities, selects the right dispatch
/// family (batched text, FA, transcribe, benchmark, media-analysis, or
/// test-echo), and delegates.
pub(super) async fn dispatch_job_with_execution_context(
    request: JobDispatchRequest,
    host: &DispatchHostContext,
    execution: &RunnerExecutionContext,
) -> Result<(), crate::error::ServerError> {
    let sink = host.sink().clone();
    let JobDispatchRequest {
        job,
        file_list,
        num_workers,
        rev_job_ids,
    } = request;
    let job_id = &job.identity.job_id;
    let correlation_id = job.identity.correlation_id.clone();
    let command = job.dispatch.command;
    let pool = &execution.pool;
    let cache = &execution.cache;
    let startup_infer_tasks = &execution.infer_tasks;
    let startup_engine_versions = &execution.engine_versions;
    let test_echo_mode = execution.test_echo_mode;
    let job_engine_overrides = job.dispatch.options.common().engine_overrides_json();

    // Capability discovery is language-agnostic — the worker reports its
    // resources.json which lists every supported language regardless of
    // which lang the worker boots with. The job-level `LanguageSpec`
    // (`Resolved(_)`, `Auto`, `PerFile`) is mapped to its `WorkerLanguage`
    // counterpart and forwarded as-is. The Python bootstrap recognises
    // `auto` and `per-file` as non-ISO sentinels and skips eager Stanza
    // model load for those — see
    // `python/batchalign/worker/_model_loading/bootstrap.py::_load_single_task`.
    // No English fallback here; the typed sum carries through.
    let capability_snapshot = match resolve_runtime_capability_snapshot(
        pool,
        startup_infer_tasks,
        startup_engine_versions,
        test_echo_mode,
        command,
        job.dispatch.lang.to_worker_language(),
        &job_engine_overrides,
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(err_msg) => {
            warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
            sink.fail_job(job_id, &err_msg, unix_now()).await;
            return Ok(());
        }
    };
    let infer_tasks = &capability_snapshot.infer_tasks;
    let engine_versions = &capability_snapshot.engine_versions;

    let all_chat = file_list.iter().all(|file| file.has_chat);
    let infer_task = infer_task_for_command(command);
    let infer_supported = infer_task.is_some_and(|task| infer_tasks.contains(&task));
    let use_infer = all_chat && infer_supported;

    if command_requires_chat_infer(command) && !use_infer {
        let required_task = infer_task.map(infer_task_name).unwrap_or("unknown");
        let err_msg = format!(
            "Rust-first dispatch requires infer task '{}' for '{}' (all_chat={}). \
             Worker advertises infer_tasks: {:?}",
            required_task, command, all_chat, infer_tasks
        );
        warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
        let failed_at = unix_now();
        sink.fail_job(job_id, &err_msg, failed_at).await;
        return Ok(());
    }

    // Special case: transcribe/transcribe_s with server-side ASR orchestration.
    // These commands take audio input (not CHAT), so they do not go through the
    // standard `use_infer` path which requires all_chat=true.
    let runner_dispatch_kind = command_runner_dispatch_kind(command);
    let use_transcribe_infer = matches!(
        runner_dispatch_kind,
        Some(RunnerDispatchKind::TranscribeAudioInfer)
    ) && infer_tasks.contains(&InferTask::Asr);
    let use_benchmark_infer = matches!(
        runner_dispatch_kind,
        Some(RunnerDispatchKind::BenchmarkAudioInfer)
    ) && infer_tasks.contains(&InferTask::Asr);
    let use_media_analysis_infer = matches!(
        runner_dispatch_kind,
        Some(RunnerDispatchKind::MediaAnalysisV2)
    ) && infer_task.is_some_and(|task| infer_tasks.contains(&task));

    if test_echo_mode {
        dispatch_test_echo_files(&job, sink.as_ref(), &file_list).await;
    } else if use_transcribe_infer {
        let engine_version = EngineVersion::from(
            engine_versions
                .get("asr")
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );

        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using server-side transcribe orchestrator"
        );

        dispatch_transcribe_command(
            &job,
            host,
            pool,
            cache,
            &engine_version,
            &rev_job_ids,
            num_workers,
        )
        .await;
    } else if use_benchmark_infer {
        let engine_version = EngineVersion::from(
            engine_versions
                .get("asr")
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );

        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using server-side benchmark orchestrator"
        );

        dispatch_benchmark_command(
            &job,
            host,
            pool,
            cache,
            &engine_version,
            &rev_job_ids,
            num_workers,
        )
        .await;
    } else if use_media_analysis_infer {
        let Some(infer_task) = infer_task else {
            tracing::error!("use_media_analysis_infer set but infer_task is None — logic error");
            return Ok(());
        };
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(infer_task))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );
        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using server-side media-analysis V2 path"
        );

        dispatch_media_analysis_command(&job, host, pool, num_workers).await;
    } else if use_infer && command == ReleasedCommand::Morphotag {
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(InferTask::Morphosyntax))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );
        let plan = BatchedInferDispatchPlan::from_job(&job, host.config());
        let gateway = PooledWorkerGateway::new(pool.clone(), cache.clone(), engine_version.clone());
        if let Err(error) = gateway
            .ensure_command_capabilities(
                command,
                job.dispatch.lang.to_worker_language(),
                &job_engine_overrides,
            )
            .await
        {
            let err_msg = format!(
                "Failed to bootstrap morphotag worker capabilities for '{}': {}",
                command, error
            );
            warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
            sink.fail_job(job_id, &err_msg, unix_now()).await;
            return Ok(());
        }
        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using recipe-owned morphotag execution path"
        );
        dispatch_morphotag_job(
            &job,
            host,
            Arc::new(gateway),
            MorphotagRuntimeOptions {
                tokenization_mode: plan.tokenization_mode,
                multilingual_policy: plan.multilingual_policy,
                mwt: Arc::new(plan.mwt),
                l2_morphotag: plan.l2_morphotag,
                respect_pos_hints: plan.respect_pos_hints,
                should_merge_abbrev: plan.should_merge_abbrev,
            },
            num_workers,
        )
        .await?;
    } else if use_infer && command == ReleasedCommand::Compare {
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(InferTask::Morphosyntax))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );
        let plan = BatchedInferDispatchPlan::from_job(&job, host.config());
        let gateway = PooledWorkerGateway::new(pool.clone(), cache.clone(), engine_version.clone());
        if let Err(error) = gateway
            .ensure_command_capabilities(
                command,
                job.dispatch.lang.to_worker_language(),
                &job_engine_overrides,
            )
            .await
        {
            let err_msg = format!(
                "Failed to bootstrap compare worker capabilities for '{}': {}",
                command, error
            );
            warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
            sink.fail_job(job_id, &err_msg, unix_now()).await;
            return Ok(());
        }
        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using recipe-owned compare execution kernel"
        );
        dispatch_compare_job(&job, host, &gateway, &plan.mwt, plan.should_merge_abbrev).await?;
    } else if use_infer && command == ReleasedCommand::Utseg {
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(InferTask::Utseg))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );
        let plan = BatchedInferDispatchPlan::from_job(&job, host.config());
        let gateway: std::sync::Arc<dyn crate::execution::WorkerGateway> = std::sync::Arc::new(
            PooledWorkerGateway::new(pool.clone(), cache.clone(), engine_version.clone()),
        );
        if let Err(error) = gateway
            .ensure_command_capabilities(
                command,
                job.dispatch.lang.to_worker_language(),
                &job_engine_overrides,
            )
            .await
        {
            let err_msg = format!(
                "Failed to bootstrap utseg worker capabilities for '{}': {}",
                command, error
            );
            warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
            sink.fail_job(job_id, &err_msg, unix_now()).await;
            return Ok(());
        }
        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using recipe-owned utseg execution path"
        );
        dispatch_utseg_job(
            &job,
            host,
            gateway,
            plan.should_merge_abbrev,
            job.dispatch.options.utseg_fallback_policy().is_allowed(),
        )
        .await?;
    } else if use_infer && command == ReleasedCommand::Translate {
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(InferTask::Translate))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );
        let plan = BatchedInferDispatchPlan::from_job(&job, host.config());
        let gateway = PooledWorkerGateway::new(pool.clone(), cache.clone(), engine_version.clone());
        if let Err(error) = gateway
            .ensure_command_capabilities(
                command,
                job.dispatch.lang.to_worker_language(),
                &job_engine_overrides,
            )
            .await
        {
            let err_msg = format!(
                "Failed to bootstrap translate worker capabilities for '{}': {}",
                command, error
            );
            warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
            sink.fail_job(job_id, &err_msg, unix_now()).await;
            return Ok(());
        }
        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using recipe-owned translate execution path"
        );
        dispatch_translate_job(&job, host, &gateway, plan.should_merge_abbrev).await?;
    } else if use_infer && command == ReleasedCommand::Coref {
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(InferTask::Coref))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );
        let plan = BatchedInferDispatchPlan::from_job(&job, host.config());
        let gateway = PooledWorkerGateway::new(pool.clone(), cache.clone(), engine_version.clone());
        if let Err(error) = gateway
            .ensure_command_capabilities(
                command,
                job.dispatch.lang.to_worker_language(),
                &job_engine_overrides,
            )
            .await
        {
            let err_msg = format!(
                "Failed to bootstrap coref worker capabilities for '{}': {}",
                command, error
            );
            warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
            sink.fail_job(job_id, &err_msg, unix_now()).await;
            return Ok(());
        }
        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using recipe-owned coref execution path"
        );
        dispatch_coref_job(&job, host, &gateway, plan.should_merge_abbrev).await?;
    } else if use_infer {
        // --- Server-side infer path ---
        // The server owns CHAT parse/cache/inject/serialize.
        // Python workers provide pure Stanza inference only.
        let Some(infer_task) = infer_task else {
            // use_infer requires infer_task.is_some() — this branch is unreachable
            // but we avoid a panic by returning early with an error log.
            tracing::error!("use_infer set but infer_task is None — logic error");
            return Ok(());
        };
        let engine_version = EngineVersion::from(
            engine_versions
                .get(infer_task_name(infer_task))
                .map(|s| s.as_str())
                .unwrap_or("unknown"),
        );

        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            engine_version = %engine_version,
            "Using server-side infer path"
        );

        match runner_dispatch_kind {
            Some(RunnerDispatchKind::ForcedAlignment) => {
                dispatch_forced_alignment_command(
                    &job,
                    host,
                    pool,
                    cache,
                    &engine_version,
                    &rev_job_ids,
                    num_workers,
                )
                .await;
            }
            Some(RunnerDispatchKind::BatchedTextInfer) => {
                dispatch_batched_text_command(&job, host, pool, cache, &engine_version).await;
            }
            other => {
                tracing::error!(
                    job_id = %job_id,
                    correlation_id = %correlation_id,
                    command = %command,
                    runner_dispatch_kind = ?other,
                    "Infer path selected for unsupported command dispatch kind"
                );
                return Ok(());
            }
        }
    } else {
        let err_msg = format!(
            "No released dispatch path remains for command '{}' (all_chat={}, infer_task={:?}, infer_supported={}). Legacy process-path fallback is retired.",
            command, all_chat, infer_task, infer_supported
        );
        warn!(job_id = %job_id, correlation_id = %correlation_id, "{}", err_msg);
        sink.fail_job(job_id, &err_msg, unix_now()).await;
        return Ok(());
    }

    Ok(())
}

fn warn_invalid_dispatch_plan(job: &RunnerJobSnapshot) {
    warn!(
        job_id = %job.identity.job_id,
        correlation_id = %job.identity.correlation_id,
        command = %job.dispatch.command,
        "Command plan could not be built from job options"
    );
}

async fn dispatch_batched_text_command(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    pool: &Arc<WorkerPool>,
    cache: &Arc<UtteranceCache>,
    engine_version: &EngineVersion,
) {
    let plan = BatchedInferDispatchPlan::from_job(job, host.config());
    dispatch_batched_infer(
        job,
        host,
        PipelineServices::new(pool, cache, engine_version),
        plan,
    )
    .await;
}

async fn dispatch_forced_alignment_command(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    pool: &Arc<WorkerPool>,
    cache: &Arc<UtteranceCache>,
    engine_version: &EngineVersion,
    rev_job_ids: &Arc<HashMap<PathBuf, RevAiJobId>>,
    num_workers: NumWorkers,
) {
    let Some(plan) = FaDispatchPlan::from_job(job, host.config()) else {
        warn_invalid_dispatch_plan(job);
        return;
    };

    dispatch_fa_infer(
        job,
        host,
        FaDispatchRuntime {
            pool: pool.clone(),
            cache: cache.clone(),
            engine_version: engine_version.clone(),
            rev_job_ids: rev_job_ids.clone(),
            num_workers,
        },
        plan,
    )
    .await;
}

async fn dispatch_transcribe_command(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    pool: &Arc<WorkerPool>,
    cache: &Arc<UtteranceCache>,
    engine_version: &EngineVersion,
    rev_job_ids: &Arc<HashMap<PathBuf, RevAiJobId>>,
    num_workers: NumWorkers,
) {
    let Some(plan) = TranscribeDispatchPlan::from_job(job, host.config()) else {
        warn_invalid_dispatch_plan(job);
        return;
    };

    dispatch_transcribe_infer(
        job,
        host,
        TranscribeDispatchRuntime {
            pool: pool.clone(),
            cache: cache.clone(),
            engine_version: engine_version.clone(),
            rev_job_ids: rev_job_ids.clone(),
            num_workers,
        },
        plan,
    )
    .await;
}

async fn dispatch_benchmark_command(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    pool: &Arc<WorkerPool>,
    cache: &Arc<UtteranceCache>,
    engine_version: &EngineVersion,
    rev_job_ids: &Arc<HashMap<PathBuf, RevAiJobId>>,
    num_workers: NumWorkers,
) {
    let Some(plan) = BenchmarkDispatchPlan::from_job(job, host.config()) else {
        warn_invalid_dispatch_plan(job);
        return;
    };

    dispatch_benchmark_infer(
        job,
        host,
        BenchmarkDispatchRuntime {
            pool: pool.clone(),
            cache: cache.clone(),
            engine_version: engine_version.clone(),
            rev_job_ids: rev_job_ids.clone(),
            num_workers,
        },
        plan,
    )
    .await;
}

async fn dispatch_media_analysis_command(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    pool: &Arc<WorkerPool>,
    num_workers: NumWorkers,
) {
    let Some(plan) = MediaAnalysisDispatchPlan::from_job(job, host.config()) else {
        warn_invalid_dispatch_plan(job);
        return;
    };

    dispatch_media_analysis_v2(
        job,
        host,
        MediaAnalysisDispatchRuntime {
            pool: pool.clone(),
            num_workers,
        },
        plan,
    )
    .await;
}

/// Resolve a runtime capability snapshot, bootstrapping live capabilities from
/// a worker if the pool has not yet detected them.
async fn resolve_runtime_capability_snapshot(
    pool: &WorkerPool,
    startup_infer_tasks: &[InferTask],
    startup_engine_versions: &BTreeMap<String, String>,
    test_echo_mode: bool,
    command: ReleasedCommand,
    lang: impl Into<crate::api::WorkerLanguage>,
    engine_overrides: &str,
) -> Result<crate::capability::WorkerCapabilitySnapshot, String> {
    if !test_echo_mode
        && pool.detected_capabilities().is_none()
        && infer_task_for_command(command).is_some()
    {
        pool.ensure_command_capabilities_with_overrides(command, lang, engine_overrides)
            .await
            .map_err(|error| {
                format!(
                    "Failed to bootstrap live worker capabilities for '{}': {}",
                    command, error
                )
            })?;
    }

    resolve_worker_capability_snapshot(
        &[],
        startup_infer_tasks,
        startup_engine_versions,
        test_echo_mode,
        pool.detected_capabilities(),
    )
    .map_err(|error| error.to_string())
}
