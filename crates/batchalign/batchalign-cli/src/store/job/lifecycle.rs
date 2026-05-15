//! Job-level lifecycle transitions.
//!
//! These methods move the job through its top-level state machine.
//! From an active state (`Queued | Running`), a job reaches one of three
//! terminal endpoints — or, on graceful shutdown, the recovery-eligible
//! `Interrupted` state:
//!
//! ```text
//! Queued → Running → { Completed | Failed | Cancelled | Interrupted }
//! ```
//!
//! `Cancelled` is reserved for user gestures (TUI cancel, HTTP DELETE/cancel)
//! and is permanent. `Interrupted` is the system-initiated counterpart written
//! by `interrupt_for_shutdown`: although `JobStatus::is_terminal()` returns
//! `true` for it, the recovery sequence at startup is special-cased to
//! transition resumable Interrupted rows back to `Queued` (see
//! `reconcile_recovered_runtime_state` here and `load_from_db` in
//! `store/queries/recovery.rs`).
//!
//! Restart preparation (`prepare_for_restart`) resets unfinished file work
//! while preserving completed results, enabling partial retry after failures.

use tokio_util::sync::CancellationToken;

use crate::api::{FileStatusKind, JobStatus, UnixTimestamp};

use super::Job;
use super::types::RecoveryDisposition;

impl Job {
    /// Return whether the job's cancellation token has been triggered.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.runtime.cancel_token.is_cancelled()
    }

    /// Return whether every terminal file currently recorded is an error.
    pub(crate) fn all_terminal_files_failed(&self) -> bool {
        let terminal: Vec<FileStatusKind> = self
            .execution
            .file_statuses
            .values()
            .filter(|file_status| file_status.status.is_terminal())
            .map(|file_status| file_status.status)
            .collect();
        !terminal.is_empty()
            && terminal
                .iter()
                .all(|status| *status == FileStatusKind::Error)
    }

    /// Return whether at least one terminal file currently recorded is
    /// an error.
    ///
    /// Pairs with [`all_terminal_files_failed`]: that predicate is the
    /// strict all-or-nothing test, this one is the partial-failure test.
    /// The job-finalization logic in `runner/execution.rs` uses this
    /// (the weaker predicate) to decide `JobStatus::Failed` so that a
    /// job with even one terminal-error file is honestly surfaced to
    /// dashboards and CLI rather than silently labelled `Completed`.
    ///
    /// Per the `JobStatus` docs at `types/status.rs`: `Completed` means
    /// "all files processed successfully", `Failed` means "one or more
    /// files encountered an unrecoverable error". This predicate is the
    /// distinguishing test between those two states.
    pub(crate) fn any_terminal_files_failed(&self) -> bool {
        self.execution
            .file_statuses
            .values()
            .filter(|file_status| file_status.status.is_terminal())
            .any(|file_status| file_status.status == FileStatusKind::Error)
    }

    /// Request cancellation and, when still active, transition to cancelled.
    pub(crate) fn request_cancellation(
        &mut self,
        completed_at: UnixTimestamp,
    ) -> Option<UnixTimestamp> {
        self.runtime.cancel_token.cancel();
        if self.execution.status.can_cancel() {
            self.execution.status = JobStatus::Cancelled;
            self.schedule.completed_at = Some(completed_at);
            self.schedule.next_eligible_at = None;
            Some(completed_at)
        } else {
            None
        }
    }

    /// Re-queue the job after memory pressure prevented dispatch.
    pub(crate) fn requeue_after_memory_gate(&mut self, retry_at: UnixTimestamp) {
        self.execution.status = JobStatus::Queued;
        self.schedule.completed_at = None;
        self.schedule.next_eligible_at = Some(retry_at);
    }

    /// Mark the job as actively running.
    pub(crate) fn mark_running(&mut self) {
        self.execution.status = JobStatus::Running;
        self.schedule.next_eligible_at = None;
    }

    /// Record the per-job worker count chosen for this run.
    pub(crate) fn record_worker_count(&mut self, num_workers: usize) {
        self.schedule.num_workers = Some(num_workers as i64);
    }

    /// Fail the job immediately with a job-level error message.
    pub(crate) fn fail(&mut self, error: &str, completed_at: UnixTimestamp) {
        self.execution.status = JobStatus::Failed;
        self.execution.error = Some(error.to_string());
        self.schedule.completed_at = Some(completed_at);
    }

    /// Mark the job interrupted by server shutdown and clear runtime claims.
    ///
    /// `Interrupted` is the correct lifecycle bit for shutdowns.  Although
    /// `JobStatus::is_terminal()` returns `true` for `Interrupted`, the
    /// recovery sequence special-cases it:
    ///
    /// 1. `db.recover_interrupted()` at startup is a SQL migration that
    ///    flips `running|queued` rows to `interrupted` — it does **not** touch
    ///    rows already written as `interrupted` (such as the rows written here).
    /// 2. `load_from_db()` then reads each row and, for any job with
    ///    `status ∈ {Interrupted, Running}`, calls
    ///    `Job::reconcile_recovered_runtime_state()` which transitions the
    ///    in-memory job (and writes back to the DB) to `Queued` if any
    ///    file is resumable.
    ///
    /// Writing `Cancelled` here (the previous behavior) skipped both steps:
    /// the row is never revisited, so an in-flight job that was running
    /// at shutdown stayed `Cancelled` forever and the user-visible dashboard
    /// showed it as a permanent user cancel even though no user pressed cancel.
    ///
    /// We deliberately set `completed_at = Some(_)` here because that is what
    /// `recover_interrupted()` itself does for the rows it migrates, and
    /// downstream queries (e.g. dashboard "completed_at" projections) expect
    /// the field to be populated for any non-active row.  The recovery flow
    /// rewrites both `status` and `completed_at` when it transitions to
    /// `Queued`.
    ///
    /// Returns `true` if the transition fired; `false` if the job was already
    /// in a terminal state and could not be moved.
    pub(crate) fn interrupt_for_shutdown(&mut self, completed_at: UnixTimestamp) -> bool {
        self.runtime.cancel_token.cancel();
        if self.execution.status.can_cancel() {
            self.execution.status = JobStatus::Interrupted;
            self.schedule.completed_at = Some(completed_at);
            self.schedule.next_eligible_at = None;
            self.clear_lease();
            self.runtime.runner_active = false;
            true
        } else {
            false
        }
    }

    /// Finalize the job after all file tasks have stopped mutating its state.
    pub(crate) fn finalize(&mut self, final_status: JobStatus, completed_at: UnixTimestamp) {
        self.execution.status = final_status;
        self.execution.batch_progress = None;
        self.schedule.completed_at = Some(completed_at);
        self.schedule.next_eligible_at = None;
        self.execution.completed_files = self
            .execution
            .file_statuses
            .values()
            .filter(|file_status| file_status.status.is_terminal())
            .count() as i64;
    }

    /// Reset the job so unfinished files may run again from queued state.
    pub(crate) fn prepare_for_restart(&mut self) {
        for file_status in self.execution.file_statuses.values_mut() {
            if file_status.status != FileStatusKind::Done {
                file_status.status = FileStatusKind::Queued;
                file_status.error = None;
                file_status.error_category = None;
                file_status.started_at = None;
                file_status.finished_at = None;
                file_status.next_eligible_at = None;
                file_status.current_attempt_id = None;
                file_status.progress_current = None;
                file_status.progress_total = None;
                file_status.progress_stage = None;
            }
        }

        self.execution.status = JobStatus::Queued;
        self.execution.error = None;
        self.schedule.completed_at = None;
        self.schedule.next_eligible_at = None;
        self.clear_lease();
        self.runtime.cancel_token = CancellationToken::new();
        self.runtime.runner_active = false;
        self.execution.completed_files = self
            .execution
            .file_statuses
            .values()
            .filter(|file_status| file_status.status == FileStatusKind::Done)
            .count() as i64;
        self.execution
            .results
            .retain(|result| result.error.is_none());
    }

    /// Reconcile a persisted interrupted/running job during startup recovery.
    pub(crate) fn reconcile_recovered_runtime_state(&mut self) -> RecoveryDisposition {
        let has_resumable = self
            .execution
            .file_statuses
            .values()
            .any(|file_status| file_status.status.is_resumable());

        if has_resumable {
            for file_status in self.execution.file_statuses.values_mut() {
                if file_status.status.is_resumable() {
                    file_status.status = FileStatusKind::Queued;
                    file_status.started_at = None;
                    file_status.finished_at = None;
                    file_status.next_eligible_at = None;
                    file_status.current_attempt_id = None;
                    file_status.progress_current = None;
                    file_status.progress_total = None;
                    file_status.progress_stage = None;
                }
            }
            self.execution.status = JobStatus::Queued;
            self.schedule.completed_at = None;
            self.schedule.next_eligible_at = None;
            self.clear_lease();
            RecoveryDisposition::Requeued
        } else {
            let all_errored = self
                .execution
                .file_statuses
                .values()
                .all(|file_status| file_status.status == FileStatusKind::Error);
            self.execution.status = if all_errored {
                JobStatus::Failed
            } else {
                JobStatus::Completed
            };
            self.execution.completed_files = self.total_files() as i64;
            self.schedule.next_eligible_at = None;
            self.clear_lease();
            if all_errored {
                RecoveryDisposition::Failed
            } else {
                RecoveryDisposition::Completed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::UnixTimestamp;
    use crate::scheduling::FailureCategory;
    use crate::store::job::test_support::{running_job_fixture, three_file_job_fixture};
    use crate::store::job::types::FileFailureRecord;

    /// Verify that `interrupt_for_shutdown` writes `JobStatus::Interrupted`
    /// (not `Cancelled`) so the recovery sequence can requeue unfinished work.
    ///
    /// This is the regression test for the 2026-04-27 investigation in which
    /// in-flight jobs were marked `Cancelled` when a server bounced; recovery
    /// never revisited `Cancelled` rows, so the jobs appeared permanently
    /// cancelled with no user action.
    #[test]
    fn interrupt_for_shutdown_writes_interrupted_not_cancelled() {
        let mut job = running_job_fixture();
        let now = UnixTimestamp(1_777_300_000.0);

        let did_transition = job.interrupt_for_shutdown(now);

        assert!(did_transition, "running job must accept the interrupt");
        assert_eq!(
            job.execution.status,
            JobStatus::Interrupted,
            "shutdown must mark the job Interrupted (recovery-eligible), not Cancelled (final)"
        );
        assert!(
            job.runtime.cancel_token.is_cancelled(),
            "the cancel token still flips so in-flight workers stop"
        );
        assert_eq!(
            job.schedule.completed_at,
            Some(now),
            "completed_at is populated to match recover_interrupted's existing convention"
        );
        assert!(!job.runtime.runner_active, "runner claim is released");
    }

    /// Regression: 2026-05-11 morphotag job `150c824a-48e`.
    ///
    /// The job submitted three files. Two terminal-errored (worker IPC
    /// protocol mismatches mid-run), one succeeded. `file_statuses` in
    /// jobs.db correctly recorded the per-file outcomes, but the overall
    /// job status was finalized as `Completed` because
    /// `all_terminal_files_failed()` reports only the all-or-nothing
    /// case. The dashboard and CLI therefore claimed the job succeeded,
    /// hiding the silent data loss for the two failed outputs.
    ///
    /// The fix predicate is `any_terminal_files_failed()`: true iff any
    /// terminal file is an error. The existing
    /// `all_terminal_files_failed()` predicate is preserved so any
    /// other caller that genuinely wants "every terminal file errored"
    /// semantics still has it. The two methods together match the
    /// `JobStatus` enum's documented contract at
    /// `crates/batchalign/src/types/status.rs` lines 27-31:
    ///
    /// > Completed = all files processed successfully
    /// > Failed = one or more files encountered an unrecoverable error
    #[test]
    fn any_terminal_files_failed_detects_partial_failure_shape() {
        let mut job = three_file_job_fixture();
        let started = UnixTimestamp(1_778_517_977.7);
        let finished_done = UnixTimestamp(1_778_518_056.4);
        let finished_error = UnixTimestamp(1_778_518_053.6);

        // 60home-3.cha succeeds (the only file with an actual output
        // on disk after the incident).
        assert!(job.mark_file_processing("60home-3.cha", started));
        assert!(job.mark_file_done("60home-3.cha", finished_done, None));

        // 65-3.cha and 65home-3.cha both terminal-error with worker
        // protocol mismatches.
        for filename in ["65-3.cha", "65home-3.cha"] {
            assert!(job.mark_file_processing(filename, started));
            assert!(job.mark_file_error(
                filename,
                &FileFailureRecord {
                    message: "worker protocol error".into(),
                    category: FailureCategory::ProviderTerminal,
                    finished_at: finished_error,
                },
            ));
        }

        assert!(
            job.any_terminal_files_failed(),
            "partial-failure must surface via any_terminal_files_failed()"
        );
        assert!(
            !job.all_terminal_files_failed(),
            "all-failed semantics must remain narrow — one done file is enough \
             for the all-failed predicate to be false"
        );
    }

    /// Edge case: no terminal files yet (everything still queued or
    /// processing). Both predicates must return false — neither
    /// "all failed" nor "any failed" applies until there's at least
    /// one terminal file to evaluate.
    #[test]
    fn any_and_all_terminal_files_failed_both_false_when_no_terminal_files() {
        let job = three_file_job_fixture();
        assert!(!job.all_terminal_files_failed());
        assert!(!job.any_terminal_files_failed());
    }

    /// Edge case: every terminal file is an error. Both predicates
    /// must return true — `any_failed` is the weaker condition and
    /// must imply `all_failed` whenever all files are terminal-error.
    #[test]
    fn any_terminal_files_failed_true_when_all_files_error() {
        let mut job = three_file_job_fixture();
        let started = UnixTimestamp(1_778_517_977.7);
        let finished = UnixTimestamp(1_778_518_056.4);
        for filename in ["65-3.cha", "60home-3.cha", "65home-3.cha"] {
            assert!(job.mark_file_processing(filename, started));
            assert!(job.mark_file_error(
                filename,
                &FileFailureRecord {
                    message: "worker error".into(),
                    category: FailureCategory::ProviderTerminal,
                    finished_at: finished,
                },
            ));
        }
        assert!(job.all_terminal_files_failed());
        assert!(job.any_terminal_files_failed());
    }
}
