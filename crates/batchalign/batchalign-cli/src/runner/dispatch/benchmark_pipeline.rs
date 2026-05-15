//! Benchmark dispatch built on the Rust-owned transcribe and compare pipelines.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::chat_ops::morphosyntax_ops::MwtDict;
use tracing::warn;

use crate::api::{EngineVersion, NumWorkers, RevAiJobId, UnixTimestamp};
use crate::benchmark::{BenchmarkRequest, process_benchmark};
use crate::cache::UtteranceCache;
use crate::pipeline::PipelineServices;
use crate::recipe_runner::runtime::{
    ChatOutputTarget, output_write_path, plan_work_units_for_job, primary_output_artifact,
    sidecar_output_artifacts, write_text_output_artifact,
};
use crate::recipe_runner::work_unit::{BenchmarkWorkUnit, PlannedWorkUnit};
use crate::runner::DispatchHostContext;
use crate::scheduling::{FailureCategory, RetryPolicy, WorkUnitKind};
use crate::store::{RunnerJobSnapshot, unix_now};
use crate::transcribe::TranscribeOptions;
use crate::worker::pool::WorkerPool;

use super::super::util::{
    FileRunTracker, FileStage, FileTaskOutcome, RunnerEventSink, classify_server_error,
    drain_supervised_file_tasks, is_retryable_worker_failure, spawn_progress_forwarder,
    spawn_supervised_file_task, user_facing_error,
};
use super::BenchmarkDispatchPlan;
use super::asr_media::{prepare_asr_media_input, preserved_media_name_for_chat};
use super::infer_batched::apply_merge_abbrev;

/// Shared runtime dependencies for top-level benchmark dispatch.
///
/// Benchmark reuses the transcribe and compare stacks, so the runtime bundle is
/// the same worker/cache context plus the file-level concurrency cap.
pub(crate) struct BenchmarkDispatchRuntime {
    /// Worker pool used for the benchmark's ASR requests.
    pub pool: Arc<WorkerPool>,
    /// Shared utterance cache used by the compare-side morphosyntax phase.
    pub cache: Arc<UtteranceCache>,
    /// Current engine version string for cache partitioning.
    pub engine_version: EngineVersion,
    /// Optional preflight Rev.AI job ids keyed by original audio path.
    pub rev_job_ids: Arc<HashMap<PathBuf, RevAiJobId>>,
    /// Maximum number of file tasks to run concurrently for this job.
    pub num_workers: NumWorkers,
}

/// Shared per-file benchmark dependencies.
///
/// Benchmark dispatch needs the same server/runtime state for every file in the
/// job. Grouping that state here keeps the per-file function focused on file
/// lifecycle rather than on a wide orchestration signature.
struct BenchmarkFileContext<'a> {
    /// Immutable runner snapshot for the current job.
    job: &'a RunnerJobSnapshot,
    /// File/job lifecycle sink for runner-side status updates.
    sink: Arc<dyn RunnerEventSink>,
    /// Shared cache/worker services for transcribe + compare.
    services: PipelineServices<'a>,
    /// Rev.AI preflight job ids keyed by the original provider audio path.
    rev_job_ids: &'a HashMap<PathBuf, RevAiJobId>,
    /// MWT dictionary shared with the compare pipeline.
    mwt: &'a MwtDict,
    /// Planned benchmark pairs keyed by main audio display path.
    planned_units: &'a HashMap<String, BenchmarkWorkUnit>,
    /// Whether output should pass through merge-abbrev before persistence.
    should_merge_abbrev: bool,
}

/// Dispatch benchmark through the Rust-owned benchmark pipeline.
pub(crate) async fn dispatch_benchmark_infer(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    runtime: BenchmarkDispatchRuntime,
    plan: BenchmarkDispatchPlan,
) {
    let BenchmarkDispatchPlan {
        kernel_plan,
        base_options,
        mwt,
        should_merge_abbrev,
    } = plan;
    let planned_units: Arc<HashMap<String, BenchmarkWorkUnit>> = {
        let sink = host.sink().clone();
        match plan_work_units_for_job(crate::api::ReleasedCommand::Benchmark, job) {
            Ok(units) => Arc::new(
                units
                    .into_iter()
                    .filter_map(|unit| match unit {
                        PlannedWorkUnit::Benchmark(benchmark) => {
                            Some((benchmark.audio.display_path.to_string(), benchmark))
                        }
                        _ => None,
                    })
                    .collect(),
            ),
            Err(error) => {
                for file in &job.pending_files {
                    FileRunTracker::new(
                        sink.as_ref(),
                        &job.identity.job_id,
                        file.filename.as_ref(),
                    )
                    .fail(
                        &format!("Benchmark planning failed: {error}"),
                        FailureCategory::Validation,
                        unix_now(),
                    )
                    .await;
                }
                return;
            }
        }
    };
    let sink = host.sink().clone();

    let file_parallelism = runtime
        .num_workers
        .0
        .max(1)
        .min(kernel_plan.file_parallelism_hint.max(1));
    let file_sem = Arc::new(tokio::sync::Semaphore::new(file_parallelism));
    let mut tasks = Vec::new();

    for file in &job.pending_files {
        if job.cancel_token.is_cancelled() {
            break;
        }

        let Ok(permit) = file_sem.clone().acquire_owned().await else {
            tracing::warn!("file semaphore closed during shutdown");
            break;
        };
        let sink = sink.clone();
        let pool = runtime.pool.clone();
        let cache = runtime.cache.clone();
        let job = job.clone();
        let engine_version = runtime.engine_version.clone();
        let mut opts = base_options.clone();
        let file = file.clone();
        let mwt = mwt.clone();
        let rev_job_ids = runtime.rev_job_ids.clone();
        let planned_units = planned_units.clone();
        let filename = file.filename.clone();

        tasks.push(spawn_supervised_file_task(
            filename,
            "benchmark file task",
            async move {
                let _permit = permit;
                let services = PipelineServices::new(&pool, &cache, &engine_version);
                let context = BenchmarkFileContext {
                    job: &job,
                    sink: sink.clone(),
                    services,
                    rev_job_ids: rev_job_ids.as_ref(),
                    mwt: &mwt,
                    planned_units: planned_units.as_ref(),
                    should_merge_abbrev,
                };
                process_one_benchmark_file(&file, &mut opts, context).await
            },
        ));
    }

    let abnormal_exits = drain_supervised_file_tasks(
        sink.as_ref(),
        &job.identity.job_id,
        &job.cancel_token,
        tasks,
    )
    .await;
    if abnormal_exits > 0 {
        warn!(
            job_id = %job.identity.job_id,
            abnormal_exits,
            "Supervised benchmark file tasks exited abnormally"
        );
    }
}

async fn process_one_benchmark_file(
    file: &crate::store::PendingJobFile,
    opts: &mut TranscribeOptions,
    context: BenchmarkFileContext<'_>,
) -> FileTaskOutcome {
    // This dispatch stays fully in Rust once the raw ASR worker capability is
    // available: resolve the gold transcript, normalize media, run the Rust
    // transcribe+compare composition, then persist the hypothesis CHAT and CSV
    // metrics artifacts.
    let BenchmarkFileContext {
        job,
        sink,
        services,
        rev_job_ids,
        mwt,
        planned_units,
        should_merge_abbrev,
    } = context;
    let job_id = &job.identity.job_id;
    let file_index = file.file_index;
    let filename = file.filename.as_ref();
    let lifecycle = FileRunTracker::new(sink.as_ref(), job_id, filename);
    let started_at = unix_now();

    lifecycle
        .begin_first_attempt(
            WorkUnitKind::FileInfer,
            started_at,
            FileStage::ResolvingAudio,
        )
        .await;

    let original_audio_path =
        resolve_benchmark_original_audio_path(&job.filesystem, file_index, filename);

    let Some(planned_unit) = planned_units.get(filename) else {
        lifecycle
            .fail(
                &format!("Benchmark planning produced no work unit for {filename}"),
                FailureCategory::Validation,
                unix_now(),
            )
            .await;
        return FileTaskOutcome::TerminalStateRecorded;
    };

    let gold_text = match tokio::fs::read_to_string(&planned_unit.gold_chat.source_path).await {
        Ok(text) => text,
        Err(err) => {
            let err_msg = format!(
                "Failed to read benchmark reference transcript {}: {err}",
                planned_unit.gold_chat.display_path
            );
            lifecycle
                .fail(&err_msg, FailureCategory::InputMissing, unix_now())
                .await;
            return FileTaskOutcome::TerminalStateRecorded;
        }
    };

    let media_name = preserved_media_name_for_chat(&original_audio_path, &original_audio_path);
    let prepared_media =
        match prepare_asr_media_input(original_audio_path, rev_job_ids, media_name, filename).await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                let err_msg = error.to_string();
                lifecycle
                    .fail(&err_msg, FailureCategory::Validation, unix_now())
                    .await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
        };
    let audio_path = prepared_media.inference_audio_path;
    opts.rev_job_id = prepared_media.rev_job_id;
    opts.media_name = prepared_media.media_name;

    let retry_policy = RetryPolicy::default();

    for attempt_number in 1..=retry_policy.max_attempts {
        if attempt_number > 1 {
            lifecycle
                .restart_attempt(WorkUnitKind::FileInfer, unix_now(), FileStage::Benchmarking)
                .await;
        } else {
            lifecycle.stage(FileStage::Benchmarking).await;
        }

        let progress_tx =
            spawn_progress_forwarder(sink.clone(), job_id.clone(), filename.to_string());

        // No silent eng fallback for benchmark either. If the job carries
        // `Auto` (uncommon for benchmark) and ASR has not yet resolved a
        // language, surface a typed error.
        let bench_lang = match job.dispatch.lang.as_resolved() {
            Some(code) => code.clone(),
            None => {
                let msg = format!(
                    "benchmark requires a resolved `--lang <iso3>`; got '{}'.",
                    job.dispatch.lang
                );
                lifecycle
                    .fail(&msg, FailureCategory::Validation, unix_now())
                    .await;
                continue;
            }
        };
        match process_benchmark(BenchmarkRequest {
            audio_path: &audio_path,
            gold_text: crate::api::ChatText::from(gold_text.as_str()),
            lang: &bench_lang,
            services,
            transcribe_options: opts,
            mwt,
            progress: Some(&progress_tx),
        })
        .await
        {
            Ok(mut outputs) => {
                lifecycle.stage(FileStage::Writing).await;
                let finished_at = unix_now();

                if should_merge_abbrev {
                    outputs.annotated_main_chat = apply_merge_abbrev(&outputs.annotated_main_chat);
                }

                let primary_output = primary_output_artifact(
                    crate::api::ReleasedCommand::Benchmark,
                    &planned_unit.audio.display_path,
                );
                // Recipe-catalog invariant: the `Benchmark` command's
                // sidecar policy includes `*.compare.csv`. The
                // catalog test in `recipe_runner/catalog.rs::tests`
                // enforces this; a missing sidecar would fail before
                // reaching here.
                #[allow(clippy::expect_used)]
                let csv_output_artifact = sidecar_output_artifacts(
                    crate::api::ReleasedCommand::Benchmark,
                    &planned_unit.audio.display_path,
                )
                .into_iter()
                .find(|artifact| artifact.display_path.as_ref().ends_with(".compare.csv"))
                .expect("benchmark command must emit a compare.csv sidecar");

                let target = ChatOutputTarget::new(
                    &job.filesystem,
                    file_index,
                    &primary_output.display_path,
                );
                if let Err(err) =
                    write_text_output_artifact(&target, &outputs.annotated_main_chat).await
                {
                    warn!(error = %err, "Failed to write benchmark CHAT output");
                }

                let csv_path = output_write_path(
                    &job.filesystem,
                    file_index,
                    &csv_output_artifact.display_path,
                );
                if let Err(err) = tokio::fs::write(&csv_path, &outputs.metrics_csv).await {
                    warn!(error = %err, "Failed to write benchmark CSV output");
                }

                lifecycle
                    .complete_with_result(
                        primary_output.display_path.clone(),
                        primary_output.content_type,
                        finished_at,
                    )
                    .await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
            Err(err) => {
                let finished_at = unix_now();
                let category = classify_server_error(&err);
                let raw_msg = format!("Benchmark failed: {err}");
                warn!(
                    job_id = %job_id,
                    filename,
                    category = %category,
                    raw_error = %raw_msg,
                    "Benchmark error (raw)"
                );
                let err_msg = user_facing_error(category, "Benchmark", filename, &raw_msg);
                let has_retry_budget = attempt_number < retry_policy.max_attempts;

                if matches!(&err, crate::error::ServerError::Worker(_))
                    && is_retryable_worker_failure(category)
                    && has_retry_budget
                {
                    let retry_number = attempt_number;
                    let backoff_ms = retry_policy.backoff_for_retry(retry_number);
                    let retry_at = UnixTimestamp(finished_at.0 + (backoff_ms.0 as f64 / 1000.0));
                    lifecycle
                        .retry(retry_at, category, &err_msg, finished_at)
                        .await;
                    continue;
                }

                lifecycle.fail(&err_msg, category, finished_at).await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
        }
    }

    FileTaskOutcome::MissingTerminalState
}

fn resolve_benchmark_original_audio_path(
    filesystem: &crate::store::RunnerFilesystemConfig,
    file_index: usize,
    filename: &str,
) -> PathBuf {
    filesystem
        .source_paths
        .get(file_index)
        .map(|cp| cp.assume_shared_filesystem().as_path().to_owned())
        // paths_mode is active for benchmark — convert ClientPath to ServerPath before joining.
        .unwrap_or_else(|| {
            filesystem
                .source_dir
                .assume_shared_filesystem()
                .join(filename)
                .as_path()
                .to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::resolve_benchmark_original_audio_path;
    use crate::store::RunnerFilesystemConfig;
    use std::path::PathBuf;

    fn filesystem_config(source_paths: Vec<&str>, source_dir: &str) -> RunnerFilesystemConfig {
        RunnerFilesystemConfig {
            paths_mode: false,
            source_paths: source_paths
                .into_iter()
                .map(batchalign_types::paths::ClientPath::from)
                .collect(),
            output_paths: Vec::new(),
            before_paths: Vec::new(),
            staging_dir: batchalign_types::paths::ServerPath::new("/tmp/staging"),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: batchalign_types::paths::ClientPath::new(source_dir),
        }
    }

    #[test]
    fn resolve_benchmark_original_audio_path_prefers_explicit_source_path() {
        let filesystem = filesystem_config(vec!["/tmp/input/clip.mp3"], "/tmp/source");

        let path = resolve_benchmark_original_audio_path(&filesystem, 0, "clip.mp3");

        assert_eq!(path, PathBuf::from("/tmp/input/clip.mp3"));
    }

    #[test]
    fn resolve_benchmark_original_audio_path_falls_back_to_source_dir() {
        let filesystem = filesystem_config(Vec::new(), "/tmp/source");

        let path = resolve_benchmark_original_audio_path(&filesystem, 0, "clip.mp3");

        assert_eq!(path, PathBuf::from("/tmp/source/clip.mp3"));
    }
}
