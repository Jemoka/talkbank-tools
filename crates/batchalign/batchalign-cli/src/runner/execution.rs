//! Job lifecycle — the top-level async tasks that own a single job's
//! execution from start to finish.
//!
//! - `job_task` — spawned background task for server-queued jobs (lease renewal
//!   + execution + cleanup).
//! - `run_server_job_attempt` / `run_direct_job` — entry points for the two
//!   host flavours (queued vs inline).
//! - `run_hosted_job` — shared core: semaphore acquire → memory reservation →
//!   preflight → dispatch → finalize.
//! - `reserve_job_execution` — host-memory coordinator interaction.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info, warn};

use crate::api::{JobId, JobStatus, RevAiJobId};
use crate::host_memory::{HostMemoryCoordinator, HostMemoryError, JobExecutionPlan};
use crate::revai::{RevAiPreflightPlan, preflight_submit_audio_paths};
use crate::scheduling::FailureCategory;
use crate::store::{
    JobCompletionSnapshot, JobStore, LeaseRenewalOutcome, RunnerJobSnapshot, unix_now,
};

use super::context::{
    DirectExecutionHost, DispatchHostContext, ExecutionReservationError, HostedJobRunOutcome,
    JobDispatchRequest, MemoryGateFailurePolicy, ServerExecutionHost,
};
use super::util::{
    FileRunTracker, RunnerEventSink, collect_preflight_audio_paths, compute_job_workers,
    force_terminal_file_states, preflight_validate_media, should_preflight,
};

/// Build the future that owns one background job lifecycle.
///
/// Each invocation is a single dispatch attempt. If the memory gate rejects the
/// attempt (`HostedJobRunOutcome::Requeued`), `job_task` spawns a new delayed
/// `job_task` that retries after the backoff deadline. This self-scheduling loop
/// ensures a requeued job always has a future runner, preventing it from staying
/// `Queued` indefinitely and blocking new submissions for the same files.
///
/// # Implementation note: explicit boxed return type
///
/// `job_task` returns `Pin<Box<dyn Future + Send + 'static>>` rather than using
/// `async fn`. The `Requeued` branch needs to recursively call `job_task` inside
/// a `tokio::spawn` closure. With `async fn`, Rust's `Send` inference would see
/// a self-referential opaque `impl Future` and fail to prove `Send`. Making the
/// return type an explicitly-typed boxed future breaks the recursion: the inner
/// call `job_task(...)` has a concrete `Pin<Box<dyn Future + Send>>` type that
/// the compiler can verify is `Send + 'static`.
pub(crate) fn job_task(
    job_id: JobId,
    host: ServerExecutionHost,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> {
    Box::pin(async move {
        info!(job_id = %job_id, "job_task started on main runtime");
        let store_for_release = host.store.clone();
        let lease_store = host.store.clone();
        let lease_job_id = job_id.clone();
        let correlation_id = host
            .store
            .runner_snapshot(&job_id)
            .await
            .map(|snapshot| snapshot.identity.correlation_id)
            .unwrap_or_else(|| job_id.to_string().into());
        let lease_task = tokio::spawn(async move {
            let interval = std::time::Duration::from_secs(JobStore::LOCAL_LEASE_HEARTBEAT_S);
            loop {
                tokio::time::sleep(interval).await;
                if lease_store.renew_job_lease(&lease_job_id).await == LeaseRenewalOutcome::Stop {
                    break;
                }
            }
        });
        match run_server_job_attempt(&job_id, &host).await {
            Ok(super::context::HostedJobRunOutcome::Completed) => {}
            Ok(super::context::HostedJobRunOutcome::Requeued { retry_at }) => {
                // The memory gate rejected this attempt. Release the claim so the job is
                // eligible again, then re-spawn after the backoff deadline. Without this,
                // the job stays Queued with next_eligible_at set but no runner —
                // permanently blocking new submissions for the same files.
                let delay_secs = (retry_at.0 - unix_now().0).max(0.0);
                let host_retry = host.clone();
                let job_id_retry = job_id.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs_f64(delay_secs)).await;
                    job_task(job_id_retry, host_retry).await;
                });
            }
            Err(e) => {
                error!(
                    job_id = %job_id,
                    correlation_id = %correlation_id,
                    error = %e,
                    "Job failed with server error"
                );
            }
        }
        lease_task.abort();
        store_for_release.release_runner_claim(&job_id).await;
    })
}

/// Run the server-owned processing lifecycle for one job.
///
/// Scoped under `CURRENT_JOB_ID` so dispatch-site `TrackerGuard`s
/// can register against this job for cancel-driven worker shutdown.
pub(crate) async fn run_server_job_attempt(
    job_id: &JobId,
    host: &ServerExecutionHost,
) -> Result<HostedJobRunOutcome, crate::error::ServerError> {
    crate::worker::pool::job_tracker::CURRENT_JOB_ID
        .scope(job_id.clone(), async {
            run_hosted_job(
                job_id,
                &host.store,
                &host.engine,
                MemoryGateFailurePolicy::Queued {
                    orchestrator: host.orchestrator.clone(),
                },
            )
            .await
        })
        .await
}

/// Run one direct inline job through the shared execution engine.
pub(crate) async fn run_direct_job(
    job_id: &JobId,
    host: &DirectExecutionHost,
) -> Result<(), crate::error::ServerError> {
    match run_hosted_job(
        job_id,
        &host.store,
        &host.engine,
        MemoryGateFailurePolicy::FailJob,
    )
    .await?
    {
        HostedJobRunOutcome::Completed => Ok(()),
        HostedJobRunOutcome::Requeued { retry_at } => Err(crate::error::ServerError::Persistence(
            format!("direct execution unexpectedly requeued job {job_id} until {retry_at}"),
        )),
    }
}

async fn run_hosted_job(
    job_id: &JobId,
    store: &Arc<JobStore>,
    engine: &super::context::ExecutionEngine,
    memory_gate_policy: MemoryGateFailurePolicy,
) -> Result<HostedJobRunOutcome, crate::error::ServerError> {
    let pool = &engine.context.pool;
    let host_context = DispatchHostContext::from_store(store.clone());
    let sink = host_context.sink().clone();

    let Some(job) = store.runner_snapshot(job_id).await else {
        // Per-host Temporal task queues make this branch unreachable in a
        // correctly configured fleet. Surface a typed error (not silent
        // `Ok(Completed)`) so a shared-queue misconfiguration or concurrent
        // store truncation fails loudly through Temporal's activity handler.
        return Err(crate::error::ServerError::JobNotInLocalStore(
            job_id.clone(),
        ));
    };
    let job = Arc::new(job);
    let correlation_id = job.identity.correlation_id.clone();

    if job.cancel_token.is_cancelled() {
        return Ok(HostedJobRunOutcome::Completed);
    }

    // Acquire semaphore (blocks if too many concurrent jobs).
    // Lifetime invariant: `store` owns the job-slot semaphore for
    // the daemon's lifetime; the only path that closes the
    // semaphore is daemon shutdown, by which point this task is
    // already cancelled or aborted.
    #[allow(clippy::expect_used)]
    let _permit = store.acquire_job_slot().await.expect("semaphore closed");

    // Check cancellation after acquiring permit
    if job.cancel_token.is_cancelled() {
        return Ok(HostedJobRunOutcome::Completed);
    }

    // Collect file list to process
    let mut file_list = job.pending_files.clone();
    let command = job.dispatch.command;
    let execution_plan = match reserve_job_execution(&host_context, job_id, &job).await {
        Ok(plan) => plan,
        Err(ExecutionReservationError::Capacity {
            requested_workers,
            error,
        }) => match &memory_gate_policy {
            MemoryGateFailurePolicy::Queued { orchestrator } => {
                let disposition = orchestrator
                    .handle_memory_gate_rejection(&sink, job_id, requested_workers, &error)
                    .await?;
                match disposition {
                    super::context::MemoryGateRejectionDisposition::Requeued { retry_at } => {
                        warn!(
                            job_id = %job_id,
                            correlation_id = %correlation_id,
                            requested_workers = requested_workers.0,
                            error = %error,
                            retry_at = %retry_at,
                            "Re-queueing job after host-memory capacity rejection"
                        );
                        return Ok(HostedJobRunOutcome::Requeued { retry_at });
                    }
                }
            }
            MemoryGateFailurePolicy::FailJob => {
                let message = error.to_string();
                sink.bump_memory_gate_aborts().await;
                sink.fail_job(job_id, &message, unix_now()).await;
                return Err(crate::error::ServerError::MemoryPressure(message));
            }
        },
        Err(ExecutionReservationError::Fatal(error)) => {
            let message = error.to_string();
            sink.fail_job(job_id, &message, unix_now()).await;
            return Err(error);
        }
    };
    let num_workers = execution_plan.granted_workers;
    let _job_memory_lease = execution_plan.lease;

    // Mark as running only after job execution memory has been reserved.
    sink.mark_job_running(job_id).await;

    // Record on job and DB
    sink.record_job_worker_count(job_id, num_workers.0).await;

    info!(
        job_id = %job_id,
        correlation_id = %correlation_id,
        num_files = file_list.len(),
        requested_workers = execution_plan.requested_workers.0,
        num_workers = num_workers.0,
        reserved_mb = execution_plan.reserved_mb.0,
        command = %command,
        "Processing files"
    );

    // Pre-validate media files (paths_mode only) to fail fast before worker dispatch.
    let media_failures = preflight_validate_media(
        &file_list,
        &job.filesystem.source_paths,
        job.filesystem.paths_mode,
    )
    .await;

    // Mark invalid files as errors immediately and collect the valid file list.
    file_list = if media_failures.is_empty() {
        file_list
    } else {
        let failed_indices =
            record_preflight_media_failures(sink.as_ref(), job_id, &file_list, &media_failures)
                .await;
        file_list
            .into_iter()
            .filter(|file| !failed_indices.contains(&file.file_index))
            .collect()
    };

    // Pre-scale workers before dispatch to avoid sequential spawn overhead.
    //
    // Skip for LazyProfile: on constrained machines (24-48 GB), workers start
    // empty and load models on demand via ensure_task. Pre-scaling would
    // eagerly spawn a worker that sits idle until files need it. The 30s cold
    // start on first dispatch is acceptable on these machines.
    //
    // For Profile mode (large/fleet), pre-scaling is an optimization that
    // amortizes cold start across concurrent files.
    let bootstrap_mode = pool.bootstrap_mode();
    // PerFile commands (morphotag, translate, coref) have no job-level
    // language — files inside one job may span many `@Languages:` headers.
    // Pre-scaling against a placeholder lang would either spawn an English
    // worker that nobody asks for (the 2026-05-03 incident) or spawn an
    // `Auto` worker that the per-file dispatch path would never key onto.
    // The pipeline spawns workers per-language lazily as it walks files;
    // skip pre-scale entirely for these commands.
    let is_per_file = job.dispatch.lang.is_per_file();
    if *num_workers > 1
        && bootstrap_mode != crate::worker::WorkerBootstrapMode::LazyProfile
        && !is_per_file
    {
        let job_engine_overrides = job.dispatch.options.dispatch_engine_overrides_json();
        let pre_scale_lang = job.dispatch.lang.to_worker_language();
        pool.pre_scale_with_overrides(
            command,
            pre_scale_lang,
            num_workers.0,
            &job_engine_overrides,
        )
        .await;
    }

    // Preflight: pre-submit audio files to Rev.AI for parallel server-side processing.
    // This collects Rev.AI job IDs that individual file tasks will poll instead of
    // re-uploading, reducing total wall-clock time by 2-5x for large batches.
    let rev_job_ids: Arc<HashMap<PathBuf, RevAiJobId>> = {
        if should_preflight(command, Some(&job.dispatch.options)) {
            let audio_paths = collect_preflight_audio_paths(command, &job, &file_list).await;

            if !audio_paths.is_empty() {
                info!(
                    job_id = %job_id,
                    correlation_id = %correlation_id,
                    num_audio_files = audio_paths.len(),
                    "Starting Rev.AI preflight submission"
                );

                let preflight_plan = RevAiPreflightPlan {
                    audio_paths,
                    lang: job.dispatch.lang.clone(),
                    num_speakers: job.dispatch.num_speakers,
                    max_concurrent: 16usize,
                };

                match preflight_submit_audio_paths(&preflight_plan).await {
                    Ok(response) => {
                        if !response.errors.is_empty() {
                            warn!(
                                job_id = %job_id,
                                num_errors = response.errors.len(),
                                "Preflight had partial errors (will fall back to per-file)"
                            );
                        }
                        info!(
                            job_id = %job_id,
                            num_submitted = response.job_ids.len(),
                            "Preflight submission complete"
                        );
                        Arc::new(response.job_ids.into_iter().collect())
                    }
                    Err(e) => {
                        warn!(
                            job_id = %job_id,
                            error = %e,
                            "Preflight failed, falling back to per-file submission"
                        );
                        Arc::new(HashMap::new())
                    }
                }
            } else {
                Arc::new(HashMap::new())
            }
        } else {
            Arc::new(HashMap::new())
        }
    };

    if let Err(error) = engine
        .dispatch_job(
            JobDispatchRequest {
                job: job.clone(),
                file_list,
                num_workers,
                rev_job_ids,
            },
            &host_context,
        )
        .await
    {
        let message = error.to_string();
        sink.fail_job(job_id, &message, unix_now()).await;
        return Err(error);
    }

    // Force unfinished files to terminal status
    let forced_errors = force_terminal_file_states(sink.as_ref(), job_id).await;

    // Set final job status
    let Some(completion) = store.completion_snapshot(job_id).await else {
        return Ok(HostedJobRunOutcome::Completed);
    };

    let completed_at = unix_now();
    let final_status = finalize_status(&completion, forced_errors);

    sink.finalize_job(job_id, final_status, completed_at).await;

    info!(
        job_id = %job_id,
        correlation_id = %correlation_id,
        status = %final_status,
        "Job finished"
    );

    Ok(HostedJobRunOutcome::Completed)
}

/// Decide a job's final terminal status from completion facts.
///
/// Pure function — extracted from the body of [`run_hosted_job`] so it
/// can be unit-tested directly.
///
/// Decision order (each row supersedes the rows below):
///
/// | Predicate                              | Resulting status     |
/// |----------------------------------------|----------------------|
/// | `completion.cancelled`                 | `Cancelled`          |
/// | `forced_errors > 0 \|\| any_failed`    | `Failed`             |
/// | otherwise                              | `Completed`          |
///
/// The middle row is the 2026-05-11 regression fix: the
/// previous predicate used `completion.all_failed`, which only fired
/// when *every* terminal file errored. A job with 2 of 3 files in
/// error and 1 success silently finalized as `Completed`, hiding the
/// per-file failures. Per the `JobStatus` doc contract at
/// `types/status.rs:27-31` (`Completed` means all files OK, `Failed`
/// means one or more files errored), `any_failed` is the correct
/// boundary.
fn finalize_status(completion: &JobCompletionSnapshot, forced_errors: usize) -> JobStatus {
    if completion.cancelled {
        JobStatus::Cancelled
    } else if forced_errors > 0 || completion.any_failed {
        JobStatus::Failed
    } else {
        JobStatus::Completed
    }
}

/// Reserve host memory for the job and compute the execution plan (worker
/// count, memory budget).
async fn reserve_job_execution(
    host: &DispatchHostContext,
    job_id: &JobId,
    job: &RunnerJobSnapshot,
) -> Result<JobExecutionPlan, ExecutionReservationError> {
    let command = job.dispatch.command;
    let requested_workers = compute_job_workers(
        command,
        job.pending_files.len(),
        host.effective_config(),
        host.config(),
    );
    let coordinator = HostMemoryCoordinator::from_server_config(host.config());
    let job_label = format!(
        "job-execution:{}:{}:{}",
        job_id,
        command,
        job.dispatch.lang.to_worker_language()
    );
    let timeout = Duration::from_secs(host.config().memory_gate_timeout_s);
    let poll_interval = Duration::from_secs(host.config().memory_gate_poll_s.max(1));
    let plan = tokio::task::spawn_blocking(move || {
        coordinator.wait_for_job_execution_plan(
            command,
            requested_workers,
            &job_label,
            timeout,
            poll_interval,
        )
    })
    .await
    .map_err(|error| {
        ExecutionReservationError::Fatal(crate::error::ServerError::Persistence(format!(
            "host-memory planner task failed for job {job_id}: {error}"
        )))
    })?;

    match plan {
        Ok(plan) => Ok(plan),
        Err(
            error @ (HostMemoryError::CapacityRejected { .. } | HostMemoryError::TimedOut { .. }),
        ) => Err(ExecutionReservationError::Capacity {
            requested_workers,
            error,
        }),
        Err(error) => Err(ExecutionReservationError::Fatal(
            crate::error::ServerError::Persistence(format!(
                "host-memory coordinator failed for job {job_id}: {error}"
            )),
        )),
    }
}

/// Record media-prevalidation failures as explicit setup attempts before the
/// job enters any concrete dispatch path.
pub(super) async fn record_preflight_media_failures(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    file_list: &[crate::store::PendingJobFile],
    media_failures: &HashMap<usize, String>,
) -> HashSet<usize> {
    let now = unix_now();
    let mut failed_indices = HashSet::with_capacity(media_failures.len());

    for (&idx, err_msg) in media_failures {
        failed_indices.insert(idx);
        if let Some(file) = file_list.iter().find(|file| file.file_index == idx) {
            FileRunTracker::new(sink, job_id, &file.filename)
                .record_setup_failure(now, err_msg, FailureCategory::Validation, now)
                .await;
        }
    }

    failed_indices
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use tokio::sync::broadcast;

    /// 2026-05-11 morphotag job `150c824a-48e`: 3 files, 1 done,
    /// 2 errored, no cancellation, no forced terminal-state errors.
    /// Before this fix the job finalized as `Completed`; now it must
    /// be `Failed`.
    #[test]
    fn finalize_status_marks_partial_failure_as_failed() {
        let completion = JobCompletionSnapshot {
            cancelled: false,
            all_failed: false,
            any_failed: true,
        };
        assert_eq!(finalize_status(&completion, 0), JobStatus::Failed);
    }

    /// Happy path: every terminal file succeeded.
    #[test]
    fn finalize_status_marks_all_done_as_completed() {
        let completion = JobCompletionSnapshot {
            cancelled: false,
            all_failed: false,
            any_failed: false,
        };
        assert_eq!(finalize_status(&completion, 0), JobStatus::Completed);
    }

    /// Worst case: every terminal file errored.
    #[test]
    fn finalize_status_marks_all_failed_as_failed() {
        let completion = JobCompletionSnapshot {
            cancelled: false,
            all_failed: true,
            any_failed: true,
        };
        assert_eq!(finalize_status(&completion, 0), JobStatus::Failed);
    }

    /// Cancellation wins over file outcomes — even if some files
    /// errored, a cancelled job is `Cancelled`, not `Failed`, so the
    /// user's gesture is preserved and the job remains restartable
    /// per the `JobStatus::can_restart` contract.
    #[test]
    fn finalize_status_cancellation_supersedes_failures() {
        let completion = JobCompletionSnapshot {
            cancelled: true,
            all_failed: false,
            any_failed: true,
        };
        assert_eq!(finalize_status(&completion, 0), JobStatus::Cancelled);
    }

    /// `forced_errors > 0` means `force_terminal_file_states` had to
    /// push at least one file from a non-terminal state into an error
    /// — that's a job-level abnormality independent of per-file
    /// outcomes. Surface as `Failed`.
    #[test]
    fn finalize_status_forced_errors_promote_to_failed() {
        let completion = JobCompletionSnapshot {
            cancelled: false,
            all_failed: false,
            any_failed: false,
        };
        assert_eq!(finalize_status(&completion, 1), JobStatus::Failed);
    }

    use crate::api::NumWorkers;
    use crate::cache::UtteranceCache;
    use crate::config::{RuntimeLayout, ServerConfig};
    use crate::db::JobDB;
    use crate::runner::context::{
        ExecutionEngine, MemoryGateRejectionDisposition, QueuedJobOrchestrator,
        RunnerExecutionContext, ServerExecutionHost,
    };
    use crate::worker::pool::{PoolConfig, WorkerPool};
    use crate::ws::BROADCAST_CAPACITY;

    struct UnreachableOrchestrator;

    #[async_trait]
    impl QueuedJobOrchestrator for UnreachableOrchestrator {
        async fn handle_memory_gate_rejection(
            &self,
            _sink: &Arc<dyn RunnerEventSink>,
            _job_id: &JobId,
            _requested_workers: NumWorkers,
            _error: &HostMemoryError,
        ) -> Result<MemoryGateRejectionDisposition, crate::error::ServerError> {
            panic!("memory-gate path must not be reached when the job is missing from the store")
        }
    }

    async fn build_empty_host(tempdir: &std::path::Path) -> ServerExecutionHost {
        let layout = RuntimeLayout::from_state_dir(tempdir.join("state"));
        std::fs::create_dir_all(layout.state_dir()).expect("create state dir");
        let db = Arc::new(
            JobDB::open_with_layout(&layout, Some(layout.state_dir()))
                .await
                .expect("open empty job db"),
        );
        let (ws_tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let config = ServerConfig {
            // `None` falls through to the tier-derived headroom.
            memory_gate_mb: None,
            ..ServerConfig::default()
        };
        let store = Arc::new(JobStore::new(config, Some(db), ws_tx));
        let pool = Arc::new(WorkerPool::new(PoolConfig {
            test_echo: true,
            ..Default::default()
        }));
        pool.start_background_tasks();
        let cache = Arc::new(
            UtteranceCache::sqlite(Some(tempdir.join("cache")))
                .await
                .expect("open utterance cache"),
        );
        let engine = ExecutionEngine::new(RunnerExecutionContext::new(
            pool,
            cache,
            Vec::new(),
            BTreeMap::new(),
            true,
        ));
        ServerExecutionHost::new(store, engine, Arc::new(UnreachableOrchestrator))
    }

    /// A foreign job — one not persisted in this server's `JobStore` — must
    /// never produce a silent `Ok(HostedJobRunOutcome::Completed)` from
    /// `run_server_job_attempt`. Per-host Temporal task queues make this
    /// architecturally impossible in production; the test guards against a
    /// regression that reintroduces a shared queue.
    #[tokio::test]
    async fn run_server_job_attempt_does_not_silently_complete_foreign_job() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let host = build_empty_host(tempdir.path()).await;

        let foreign_job = JobId::from("foreign-to-this-store");
        let outcome = run_server_job_attempt(&foreign_job, &host).await;

        assert!(
            !matches!(outcome, Ok(HostedJobRunOutcome::Completed)),
            "run_server_job_attempt reported Completed for a job missing from \
             the local store (outcome: {outcome:?}); expected a typed error."
        );
    }
}
