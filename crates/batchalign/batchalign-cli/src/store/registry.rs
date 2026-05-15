//! Private in-memory job-registry implementation for [`JobStore`](super::JobStore).
//!
//! The rest of the server should think in terms of store operations and job
//! transitions, not in terms of "there is a `Mutex<HashMap<...>>` somewhere."
//! This type keeps the raw collection plus lock local to the store layer and
//! exposes only projection/mutation helpers.

use std::collections::HashMap;

use batchalign_types::paths::ServerPath;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

use crate::api::{
    DisplayPath, FileStatusEntry, JobId, JobInfo, JobListItem, JobStatus, NodeId, UnixTimestamp,
};
use crate::error::ServerError;
#[cfg(test)]
use crate::queue::QueuePoll;
use crate::scheduling::LeaseRecord;

use super::JobDetail;
#[cfg(test)]
use super::job::JobLeaseState;
use super::job::{
    CompletedFileOutput, FileFailureRecord, FileProgressRecord, FileRetryRecord, Job,
    RunnerJobSnapshot, find_conflicts,
};

/// One claimed lease that should be mirrored into durable storage.
///
/// Currently used only by the test-only queue-claim path.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct ClaimedLeaseRecord {
    /// Job whose runner claim was acquired.
    pub(crate) job_id: JobId,
    /// Lease details that should be written to SQLite.
    pub(crate) lease: LeaseRecord,
}

/// Result of claiming all currently runnable queued jobs.
///
/// Currently used only by the test-only queue-claim path.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ClaimedQueuePoll {
    /// Queue wakeup information for the dispatcher loop.
    pub(crate) poll: QueuePoll,
    /// Leases that must be mirrored into SQLite after the lock is released.
    pub(crate) claimed_leases: Vec<ClaimedLeaseRecord>,
}

/// Minimal facts the runner needs when deciding a job's terminal outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JobCompletionSnapshot {
    /// Whether the job's cancellation token has been triggered.
    pub cancelled: bool,
    /// Whether every terminal file currently recorded is an error.
    ///
    /// Strict all-or-nothing test. Useful for diagnostics. The
    /// `run_hosted_job` finalizer uses [`Self::any_failed`] for the
    /// `Completed` vs `Failed` decision — see the 2026-05-11
    /// regression in `runner/execution.rs`.
    pub all_failed: bool,
    /// Whether at least one terminal file currently recorded is an
    /// error.
    ///
    /// This is the predicate that drives `JobStatus::Failed`
    /// vs `Completed` for a job that has reached its terminal point.
    /// The `JobStatus` enum docstring at `types/status.rs:27-31`
    /// defines `Completed` as "all files processed successfully" and
    /// `Failed` as "one or more files encountered an unrecoverable
    /// error" — `any_failed` is the boundary between those two states.
    pub any_failed: bool,
}

/// One file update projected into the WebSocket/dashboard shape.
#[derive(Debug, Clone)]
pub(crate) struct FileUpdateProjection {
    /// Parent job that owns the updated file.
    pub job_id: JobId,
    /// Current API-facing file status entry.
    pub file: FileStatusEntry,
    /// Running total of terminal files for the parent job.
    pub completed_files: i64,
}

/// Restart result that carries both REST and WebSocket projections.
#[derive(Debug, Clone)]
pub(crate) struct JobRestartProjection {
    /// REST response payload for the restart endpoint.
    pub info: JobInfo,
    /// Summary row that should be published to WebSocket listeners.
    pub job_update: JobListItem,
}

/// One read-only registry operation sent to the owned actor task.
type InspectOp = Box<dyn FnOnce(&HashMap<JobId, Job>) + Send + 'static>;

/// One mutable registry operation sent to the owned actor task.
type MutateOp = Box<dyn FnOnce(&mut HashMap<JobId, Job>) + Send + 'static>;

/// Command delivered to the job-registry actor.
enum RegistryCommand {
    /// Run one read-only projection against the current job map.
    Inspect {
        /// Closure that owns the full read-side work plus any reply handling.
        op: InspectOp,
    },
    /// Run one in-place mutation against the current job map.
    Mutate {
        /// Closure that owns the full write-side work plus any reply handling.
        op: MutateOp,
    },
}

/// In-memory registry of all known jobs.
///
/// `JobStore` owns one registry instance for the lifetime of the server. The
/// registry intentionally does not know about SQLite, WebSockets, or route
/// semantics; it is only the in-memory ownership boundary around the current
/// set of jobs. The collection itself now lives inside one owned actor task, so
/// callers talk to a message boundary rather than directly sharing a mutex.
pub(crate) struct JobRegistry {
    commands: UnboundedSender<RegistryCommand>,
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl JobRegistry {
    /// Create an empty registry.
    pub(crate) fn new() -> Self {
        let (commands, receiver) = unbounded_channel();
        tokio::spawn(run_registry(receiver));
        Self { commands }
    }

    fn send(&self, command: RegistryCommand) {
        // Actor-lifetime invariant: `run_registry` is spawned at
        // `JobRegistry::new` (line 125) and runs for the lifetime of
        // the daemon process. Its `mpsc::Receiver` is held alive by
        // that spawned task; an `mpsc::Sender::send` only fails when
        // every Receiver has been dropped, which can only happen when
        // the actor task has terminated. The actor never terminates
        // voluntarily — it loops on `receiver.recv().await` until the
        // process is killed. Reaching the expect therefore means the
        // actor has crashed, which is itself a panic-level bug.
        #[allow(clippy::expect_used)]
        self.commands
            .send(command)
            .expect("job registry actor dropped unexpectedly");
    }

    /// Execute a read-only projection over the current job map.
    ///
    /// This remains available for the few bulk operations that genuinely need
    /// the entire registry at once, such as crash recovery. New store code
    /// should prefer the named methods below so the coordinator logic remains
    /// readable in terms of jobs and transitions rather than map plumbing.
    pub(crate) async fn inspect_all<R>(
        &self,
        f: impl FnOnce(&HashMap<JobId, Job>) -> R + Send + 'static,
    ) -> R
    where
        R: Send + 'static,
    {
        let (reply, receiver) = oneshot::channel();
        self.send(RegistryCommand::Inspect {
            op: Box::new(move |jobs| {
                let _ = reply.send(f(jobs));
            }),
        });
        // oneshot reply invariant: `RegistryCommand::Inspect` was just
        // delivered to the actor (above); the actor's per-command
        // handler always sends on `reply` before continuing to the
        // next command. The expect therefore covers the same crash
        // case as `send` above.
        #[allow(clippy::expect_used)]
        receiver
            .await
            .expect("job registry actor dropped before read reply")
    }

    /// Execute one in-place mutation over the current job map.
    ///
    /// Like [`JobRegistry::inspect_all`], this is the low-level escape hatch
    /// for bulk operations that truly need collection-wide ownership.
    pub(crate) async fn mutate_all<R>(
        &self,
        f: impl FnOnce(&mut HashMap<JobId, Job>) -> R + Send + 'static,
    ) -> R
    where
        R: Send + 'static,
    {
        let (reply, receiver) = oneshot::channel();
        self.send(RegistryCommand::Mutate {
            op: Box::new(move |jobs| {
                let _ = reply.send(f(jobs));
            }),
        });
        // Same actor-lifetime invariant as `inspect_all` above.
        #[allow(clippy::expect_used)]
        receiver
            .await
            .expect("job registry actor dropped before write reply")
    }

    /// Project one job by ID.
    ///
    /// Store queries use this for genuinely job-local reads while keeping the
    /// underlying map private to the registry layer.
    async fn project_job<R>(
        &self,
        job_id: JobId,
        f: impl FnOnce(&Job) -> R + Send + 'static,
    ) -> Option<R>
    where
        R: Send + 'static,
    {
        self.inspect_all(move |jobs| jobs.get(&job_id).map(f)).await
    }

    /// Mutate one job by ID.
    ///
    /// This is the per-job mutable counterpart to
    /// [`JobRegistry::project_job`]. The caller describes the transition; the
    /// registry owns the lock.
    pub(crate) async fn update_job<R>(
        &self,
        job_id: JobId,
        f: impl FnOnce(&mut Job) -> R + Send + 'static,
    ) -> Option<R>
    where
        R: Send + 'static,
    {
        self.mutate_all(move |jobs| jobs.get_mut(&job_id).map(f))
            .await
    }

    /// Remove one job from the registry and hand ownership to the caller.
    async fn remove_job<R>(
        &self,
        job_id: JobId,
        f: impl FnOnce(Job) -> R + Send + 'static,
    ) -> Option<R>
    where
        R: Send + 'static,
    {
        self.mutate_all(move |jobs| jobs.remove(&job_id).map(f))
            .await
    }

    /// Build the current file-update projection for one file inside one job.
    ///
    /// This keeps the file notification shape close to the registry so query
    /// modules can talk in terms of `FileUpdateProjection` rather than holding a
    /// borrowed `Job` just to serialize one nested field.
    fn file_update_projection(job: &Job, filename: &str) -> Option<FileUpdateProjection> {
        let file = job.execution.file_statuses.get(filename)?.to_entry();
        Some(FileUpdateProjection {
            job_id: job.identity.job_id.clone(),
            file,
            completed_files: job.execution.completed_files,
        })
    }

    /// Insert a newly submitted job after conflict detection.
    pub(crate) async fn insert_checked(&self, job: Job) -> Result<(), ServerError> {
        let job_id = job.identity.job_id.clone();
        self.mutate_all(move |jobs| -> Result<(), ServerError> {
            let conflicts = find_conflicts(jobs, &job);
            if !conflicts.is_empty() {
                let filenames: Vec<&str> = conflicts
                    .iter()
                    .map(|conflict| conflict.filename.as_ref())
                    .collect();
                let message =
                    format!("Files already being processed by an active job: {filenames:?}");
                let details = conflicts
                    .into_iter()
                    .map(|conflict| crate::error::ConflictDetail {
                        filename: conflict.filename,
                        job_id: conflict.job_id,
                        command: conflict.command,
                        status: conflict.status,
                    })
                    .collect();
                return Err(ServerError::JobConflict {
                    message,
                    conflicts: details,
                });
            }

            jobs.insert(job_id, job);
            Ok(())
        })
        .await
    }

    /// Project one job into the API response type used by `GET /jobs/:id`.
    pub(crate) async fn job_info(&self, job_id: &JobId) -> Option<JobInfo> {
        self.project_job(job_id.clone(), |job| job.to_info()).await
    }

    /// Return all jobs in newest-first order for dashboard and CLI listing.
    pub(crate) async fn list_items(&self) -> Vec<JobListItem> {
        self.inspect_all(|jobs| {
            let mut items: Vec<JobListItem> = jobs.values().map(|job| job.to_list_item()).collect();
            items.sort_by(|a, b| {
                let ta = a.submitted_at.as_deref().unwrap_or("");
                let tb = b.submitted_at.as_deref().unwrap_or("");
                tb.cmp(ta)
            });
            items
        })
        .await
    }

    /// Request cancellation for one job and return any terminal timestamp that
    /// should be mirrored into SQLite.
    pub(crate) async fn request_cancellation(
        &self,
        job_id: &JobId,
        cancelled_at: UnixTimestamp,
    ) -> Option<Option<UnixTimestamp>> {
        self.update_job(job_id.clone(), move |job| {
            job.request_cancellation(cancelled_at)
        })
        .await
    }

    /// Project the most-recent cancel attempt's metadata onto the in-memory
    /// `Job` so `JobInfo::last_cancelled_*` reflects it without a DB round
    /// trip. Called by the store cancel pathway right after the audit-table
    /// row is persisted. Returns `true` if the job was found, `false` if it
    /// was already evicted.
    pub(crate) async fn set_last_cancel(
        &self,
        job_id: &JobId,
        info: crate::store::JobLastCancelInfo,
    ) -> bool {
        self.update_job(job_id.clone(), move |job| {
            job.schedule.last_cancel = Some(info);
        })
        .await
        .is_some()
    }

    /// Remove one job and return its staging directory for cleanup.
    pub(crate) async fn remove_staging_dir(&self, job_id: &JobId) -> Option<ServerPath> {
        self.remove_job(job_id.clone(), |job| job.filesystem.staging_dir)
            .await
    }

    /// Reset a failed or cancelled job back to queued state.
    ///
    /// The returned projection carries both the REST payload for the restart
    /// endpoint and the summary row that should be broadcast to live clients.
    pub(crate) async fn restart_job(
        &self,
        job_id: &JobId,
    ) -> Option<Result<JobRestartProjection, ServerError>> {
        let job_id_for_message = job_id.clone();
        self.update_job(job_id.clone(), move |job| -> Result<JobRestartProjection, ServerError> {
            if !job.execution.status.can_restart() {
                return Err(ServerError::JobConflict {
                    message: format!(
                        "Job {job_id_for_message} is {} — only cancelled or failed jobs can be restarted.",
                        job.execution.status
                    ),
                    conflicts: Vec::new(),
                });
            }

            job.prepare_for_restart();
            Ok(JobRestartProjection {
                info: job.to_info(),
                job_update: job.to_list_item(),
            })
        })
        .await
    }

    /// Return whether one job is currently running.
    pub(crate) async fn is_running(&self, job_id: &JobId) -> Option<bool> {
        self.project_job(job_id.clone(), |job| {
            job.execution.status == JobStatus::Running
        })
        .await
    }

    /// Return whether the local dispatcher currently holds an active runner claim.
    #[cfg(test)]
    pub(crate) async fn runner_claim_active(&self, job_id: &JobId) -> Option<bool> {
        self.project_job(job_id.clone(), |job| job.runtime.runner_active)
            .await
    }

    /// Return a cloned snapshot of the current lease state for one job.
    #[cfg(test)]
    pub(crate) async fn lease_state(&self, job_id: &JobId) -> Option<JobLeaseState> {
        self.project_job(job_id.clone(), |job| job.schedule.lease.clone())
            .await
    }

    /// Return the current job status for one job.
    pub(crate) async fn job_status(&self, job_id: &JobId) -> Option<JobStatus> {
        self.project_job(job_id.clone(), |job| job.execution.status)
            .await
    }

    /// Count how many jobs are currently running.
    pub(crate) async fn active_jobs(&self) -> i64 {
        self.inspect_all(|jobs| {
            jobs.values()
                .filter(|job| job.execution.status == JobStatus::Running)
                .count() as i64
        })
        .await
    }

    /// Re-queue one job after a memory-gate rejection and return the new
    /// summary-row projection for WebSocket listeners.
    pub(crate) async fn requeue_after_memory_gate(
        &self,
        job_id: &JobId,
        retry_at: UnixTimestamp,
    ) -> Option<JobListItem> {
        self.update_job(job_id.clone(), move |job| {
            job.requeue_after_memory_gate(retry_at);
            job.to_list_item()
        })
        .await
    }

    /// Mark one queued job as running and return its updated summary row.
    pub(crate) async fn mark_job_running(&self, job_id: &JobId) -> Option<JobListItem> {
        self.update_job(job_id.clone(), |job| {
            job.mark_running();
            job.to_list_item()
        })
        .await
    }

    /// Record the runner worker-count choice for one job.
    pub(crate) async fn record_job_worker_count(&self, job_id: &JobId, num_workers: usize) -> bool {
        self.update_job(job_id.clone(), move |job| {
            job.record_worker_count(num_workers)
        })
        .await
        .is_some()
    }

    /// Fail one job immediately and return the summary row for notifications.
    pub(crate) async fn fail_job(
        &self,
        job_id: &JobId,
        error: &str,
        completed_at: UnixTimestamp,
    ) -> Option<JobListItem> {
        let error = error.to_string();
        self.update_job(job_id.clone(), move |job| {
            job.fail(&error, completed_at);
            job.to_list_item()
        })
        .await
    }

    /// Return the minimal facts needed to compute the final terminal status.
    pub(crate) async fn completion_snapshot(
        &self,
        job_id: &JobId,
    ) -> Option<JobCompletionSnapshot> {
        self.project_job(job_id.clone(), |job| JobCompletionSnapshot {
            cancelled: job.is_cancelled(),
            all_failed: job.all_terminal_files_failed(),
            any_failed: job.any_terminal_files_failed(),
        })
        .await
    }

    /// Finalize one job and return the summary row for live notifications.
    pub(crate) async fn finalize_job(
        &self,
        job_id: &JobId,
        final_status: JobStatus,
        completed_at: UnixTimestamp,
    ) -> Option<JobListItem> {
        self.update_job(job_id.clone(), move |job| {
            job.finalize(final_status, completed_at);
            job.to_list_item()
        })
        .await
    }

    /// Project the download-facing detail view for one job.
    pub(crate) async fn job_detail(&self, job_id: &JobId) -> Option<JobDetail> {
        self.project_job(job_id.clone(), |job| JobDetail {
            status: job.execution.status,
            paths_mode: job.filesystem.paths_mode,
            staging_dir: job.filesystem.staging_dir.clone(),
            results: job.execution.results.clone(),
            file_statuses: job
                .execution
                .file_statuses
                .values()
                .map(|file_status| file_status.to_entry())
                .collect(),
        })
        .await
    }

    /// Interrupt every active job for graceful server shutdown, returning how many
    /// were updated.
    ///
    /// Uses `Job::interrupt_for_shutdown` so jobs land in `JobStatus::Interrupted`
    /// (resumable via the startup recovery path) rather than `JobStatus::Cancelled`
    /// (terminal user-cancel state).  See `Job::interrupt_for_shutdown` doc for the
    /// full recovery-flow rationale.
    pub(crate) async fn interrupt_all_active(&self, interrupted_at: UnixTimestamp) -> usize {
        self.mutate_all(move |jobs| {
            let mut count = 0;
            for job in jobs.values_mut() {
                if job.interrupt_for_shutdown(interrupted_at) {
                    count += 1;
                }
            }
            count
        })
        .await
    }

    /// Return IDs of jobs that remain queued after startup recovery.
    pub(crate) async fn queued_job_ids(&self) -> Vec<JobId> {
        self.inspect_all(|jobs| {
            jobs.values()
                .filter(|job| job.execution.status == JobStatus::Queued)
                .map(|job| job.identity.job_id.clone())
                .collect()
        })
        .await
    }

    /// Claim every queued job that is eligible to run on this node.
    ///
    /// Currently exercised only by the test-only local queue-claim path.
    #[cfg(test)]
    pub(crate) async fn claim_ready_queued_jobs(
        &self,
        now: UnixTimestamp,
        node_id: &NodeId,
        lease_ttl_s: f64,
    ) -> ClaimedQueuePoll {
        let node_id = node_id.clone();
        self.mutate_all(move |jobs| {
            let mut ready: Vec<(f64, JobId)> = jobs
                .values()
                .filter(|job| job.ready_for_local_dispatch(now))
                .map(|job| (job.schedule.submitted_at.0, job.identity.job_id.clone()))
                .collect();
            ready.sort_by(|a, b| a.0.total_cmp(&b.0));

            let ready_job_ids: Vec<JobId> = ready.into_iter().map(|(_, job_id)| job_id).collect();
            let mut claimed_leases: Vec<ClaimedLeaseRecord> =
                Vec::with_capacity(ready_job_ids.len());
            for job_id in &ready_job_ids {
                if let Some(job) = jobs.get_mut(job_id)
                    && let Some(lease) = job.claim_for_local_dispatch(&node_id, now, lease_ttl_s)
                {
                    claimed_leases.push(ClaimedLeaseRecord {
                        job_id: job_id.clone(),
                        lease,
                    });
                }
            }

            let next_wake_at = jobs
                .values()
                .filter(|job| {
                    job.execution.status == JobStatus::Queued && !job.runtime.runner_active
                })
                .filter_map(|job| job.next_local_dispatch_wake_at(now))
                .min_by(|a, b| a.0.total_cmp(&b.0));

            ClaimedQueuePoll {
                poll: QueuePoll {
                    ready_job_ids,
                    next_wake_at,
                },
                claimed_leases,
            }
        })
        .await
    }

    /// Release the local dispatch claim on one job.
    pub(crate) async fn release_runner_claim(&self, job_id: &JobId) -> bool {
        self.update_job(job_id.clone(), |job| job.release_local_dispatch_claim())
            .await
            .is_some()
    }

    /// Renew the local queue lease for one claimed job.
    pub(crate) async fn renew_job_lease(
        &self,
        job_id: &JobId,
        node_id: &NodeId,
        now: UnixTimestamp,
        lease_ttl_s: f64,
    ) -> Option<LeaseRecord> {
        let node_id = node_id.clone();
        self.update_job(job_id.clone(), move |job| {
            job.renew_local_dispatch_lease(&node_id, now, lease_ttl_s)
        })
        .await
        .flatten()
    }

    /// Return the immutable runner-facing projection for one job.
    pub(crate) async fn runner_snapshot(&self, job_id: &JobId) -> Option<RunnerJobSnapshot> {
        self.project_job(job_id.clone(), |job| job.to_runner_snapshot())
            .await
    }

    /// Return the filenames that still need work for one job.
    pub(crate) async fn unfinished_files(&self, job_id: &JobId) -> Vec<DisplayPath> {
        self.project_job(job_id.clone(), |job| job.unfinished_files())
            .await
            .unwrap_or_default()
    }

    /// Return the current human-readable file status label for one file.
    pub(crate) async fn file_status_label(&self, job_id: &JobId, filename: &str) -> Option<String> {
        let filename = filename.to_string();
        self.project_job(job_id.clone(), move |job| job.file_status_label(&filename))
            .await
            .flatten()
    }

    /// Mark one file as processing and return its file-update projection.
    pub(crate) async fn mark_file_processing(
        &self,
        job_id: &JobId,
        filename: &str,
        started_at: UnixTimestamp,
    ) -> Option<FileUpdateProjection> {
        let filename = filename.to_string();
        self.update_job(job_id.clone(), move |job| {
            if job.mark_file_processing(&filename, started_at) {
                Self::file_update_projection(job, &filename)
            } else {
                None
            }
        })
        .await
        .flatten()
    }

    /// Mark one file as complete and return the new file-update projection.
    pub(crate) async fn mark_file_done(
        &self,
        job_id: &JobId,
        filename: &str,
        finished_at: UnixTimestamp,
        result: Option<CompletedFileOutput>,
    ) -> Option<FileUpdateProjection> {
        let filename = filename.to_string();
        self.update_job(job_id.clone(), move |job| {
            if job.mark_file_done(&filename, finished_at, result.clone()) {
                Self::file_update_projection(job, &filename)
            } else {
                None
            }
        })
        .await
        .flatten()
    }

    /// Mark one file as terminally failed and return the projected update.
    pub(crate) async fn mark_file_error(
        &self,
        job_id: &JobId,
        filename: &str,
        failure: &FileFailureRecord,
    ) -> Option<FileUpdateProjection> {
        let filename = filename.to_string();
        let failure = failure.clone();
        self.update_job(job_id.clone(), move |job| {
            if job.mark_file_error(&filename, &failure) {
                Self::file_update_projection(job, &filename)
            } else {
                None
            }
        })
        .await
        .flatten()
    }

    /// Record the start of one file attempt and return the projected update.
    pub(crate) async fn start_file_attempt(
        &self,
        job_id: &JobId,
        filename: &str,
        started_at: UnixTimestamp,
    ) -> Option<FileUpdateProjection> {
        let filename = filename.to_string();
        self.update_job(job_id.clone(), move |job| {
            if job.start_file_attempt(&filename, started_at) {
                Self::file_update_projection(job, &filename)
            } else {
                None
            }
        })
        .await
        .flatten()
    }

    /// Mark one file as waiting for retry and return the projected update.
    pub(crate) async fn mark_file_retry_pending(
        &self,
        job_id: &JobId,
        filename: &str,
        retry: &FileRetryRecord,
    ) -> Option<FileUpdateProjection> {
        let filename = filename.to_string();
        let retry = retry.clone();
        self.update_job(job_id.clone(), move |job| {
            if job.mark_file_retry_pending(&filename, &retry) {
                Self::file_update_projection(job, &filename)
            } else {
                None
            }
        })
        .await
        .flatten()
    }

    /// Clear retry-only transient fields before a fresh attempt.
    pub(crate) async fn clear_file_retry_state(&self, job_id: &JobId, filename: &str) -> bool {
        let filename = filename.to_string();
        self.update_job(job_id.clone(), move |job| {
            job.clear_file_retry_state(&filename)
        })
        .await
        .is_some()
    }

    /// Apply one progress update and return the projected file-update payload.
    pub(crate) async fn set_file_progress(
        &self,
        job_id: &JobId,
        filename: &str,
        progress: &FileProgressRecord,
    ) -> Option<FileUpdateProjection> {
        let filename = filename.to_string();
        let progress = progress.clone();
        self.update_job(job_id.clone(), move |job| {
            if job.set_file_progress(&filename, &progress) {
                Self::file_update_projection(job, &filename)
            } else {
                None
            }
        })
        .await
        .flatten()
    }

    /// Attach one newly created attempt identifier to a file status entry.
    pub(crate) async fn attach_attempt_id(
        &self,
        job_id: &JobId,
        filename: &str,
        attempt_id: String,
    ) -> bool {
        let filename = filename.to_string();
        self.update_job(job_id.clone(), move |job| {
            if let Some(file_status) = job.execution.file_statuses.get_mut(&filename) {
                file_status.current_attempt_id = Some(attempt_id.clone());
                true
            } else {
                false
            }
        })
        .await
        .unwrap_or(false)
    }

    /// Take and clear the currently active attempt id for one file.
    pub(crate) async fn take_attempt_id(&self, job_id: &JobId, filename: &str) -> Option<String> {
        let filename = filename.to_string();
        self.update_job(job_id.clone(), move |job| {
            job.execution
                .file_statuses
                .get_mut(&filename)
                .and_then(|file_status| file_status.current_attempt_id.take())
        })
        .await
        .flatten()
    }
}

/// Run the job-registry actor loop.
async fn run_registry(mut receiver: UnboundedReceiver<RegistryCommand>) {
    let mut jobs = HashMap::new();

    while let Some(command) = receiver.recv().await {
        match command {
            RegistryCommand::Inspect { op } => op(&jobs),
            RegistryCommand::Mutate { op } => op(&mut jobs),
        }
    }
}
