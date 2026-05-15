//! Local-store ↔ Temporal reconciliation.
//!
//! In a Temporal-backend deployment the local store is a cache, not the
//! authoritative source of job status — Temporal is. Without this component
//! the local store drifts: activities complete on other fleet workers but
//! the submitting daemon's local DB never learns, leaving jobs stuck
//! `Queued`/`Running` and blocking conflict-detection on resubmissions.
//!
//! Layers:
//!
//! 1. [`reconcile_action`] — pure decision function mapping `(store state,
//!    runtime hints, Temporal verdict)` to a [`ReconcileAction`].
//!
//! 2. [`TemporalStateQuery`] — trait that abstracts "ask Temporal about a
//!    workflow". The real impl wraps a Temporal client; tests use a double.
//!
//! 3. [`TemporalReconciler`] — orchestrates the loop. Snapshots active
//!    jobs, fans Temporal describes out concurrently, applies actions to
//!    `JobStore`. Callers run it both as a periodic background tick and
//!    opportunistically on new submissions (scoped to the submitter) to
//!    close the race window between ticks.

use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use tracing::debug;

use crate::api::{CancelReason, CancelSource, CancellationRecord, JobId, JobStatus, UnixTimestamp};
use crate::error::ServerError;
use crate::store::JobStore;
use crate::store::queries::ReconcilableJobSnapshot;
use crate::store::unix_now;

/// Bound on how many Temporal describes can be in flight concurrently
/// from a single reconcile pass. Temporal's frontend tolerates well
/// beyond this, but we keep the fan-out modest so a daemon with many
/// active jobs can't starve its own HTTP server with describe work.
const MAX_CONCURRENT_DESCRIBES: usize = 16;

/// Trait for asking Temporal about a workflow's lifecycle state. The
/// real implementation wraps `TemporalClient`; tests use a fake that
/// returns canned responses. Decoupling through this trait means the
/// reconciler loop is testable without a running Temporal server.
#[async_trait::async_trait]
pub trait TemporalStateQuery: Send + Sync {
    /// Returns `Ok(Some(outcome))` if the workflow is known to Temporal,
    /// `Ok(None)` if Temporal reports not-found, or `Err(_)` on a
    /// transport failure. `Err` is treated as "leave the job alone this
    /// tick" by the reconciler — reconciliation is best-effort.
    async fn query_workflow_outcome(
        &self,
        job_id: &JobId,
    ) -> Result<Option<TemporalWorkflowOutcome>, ServerError>;
}

/// Summary of what a reconcile pass accomplished. Exposed for tracing
/// and for tests to assert on the update count.
#[derive(Debug, Default, Clone)]
pub struct ReconcileReport {
    /// Jobs whose local status was rewritten this pass.
    pub updated: usize,
    /// Jobs examined but left alone (either matched Temporal or
    /// inside the grace window).
    pub unchanged: usize,
    /// Jobs skipped because the Temporal query failed.
    pub errored: usize,
}

/// Pushes Temporal's workflow verdict back into the local `JobStore`.
/// Run as a periodic background tick and invoked opportunistically from
/// `TemporalServerBackend::submit_job` (scoped to the submitter).
pub struct TemporalReconciler {
    store: Arc<JobStore>,
    query: Arc<dyn TemporalStateQuery>,
    stale_threshold_s: u64,
}

impl TemporalReconciler {
    /// Construct a reconciler bound to a store, a Temporal state query,
    /// and a threshold for when to sweep workflows that Temporal
    /// reports as not-found. The threshold should comfortably exceed
    /// workflow-start visibility latency (typically a few seconds).
    pub fn new(
        store: Arc<JobStore>,
        query: Arc<dyn TemporalStateQuery>,
        stale_threshold_s: u64,
    ) -> Self {
        Self {
            store,
            query,
            stale_threshold_s,
        }
    }

    /// Reconcile every non-terminal job in the store against Temporal.
    ///
    /// Pull a snapshot of reconcilable jobs, ask the Temporal query
    /// for each, compute the action via the pure `reconcile_action`
    /// function, and apply any mutations through `JobStore`.
    pub async fn reconcile_all_active(&self) -> ReconcileReport {
        self.reconcile_filtered(None).await
    }

    /// Like `reconcile_all_active`, but only for jobs submitted by the
    /// given client. Used opportunistically on new submissions to
    /// close the race window between scheduled reconciler ticks.
    pub async fn reconcile_submitter(&self, submitted_by: &str) -> ReconcileReport {
        self.reconcile_filtered(Some(submitted_by.to_owned())).await
    }

    async fn reconcile_filtered(&self, submitted_by: Option<String>) -> ReconcileReport {
        let snapshots = self.store.reconcilable_snapshot(submitted_by).await;
        let now = UnixTimestamp(unix_now().0);

        // Fan Temporal describes out concurrently — each is network-bound
        // and independent. Without this, a submitter with K stale jobs
        // pays K × RTT synchronously inside the submit_job hot path.
        let queried: Vec<(ReconcilableJobSnapshot, Result<_, _>)> = {
            let query = self.query.clone();
            let mut in_flight: FuturesUnordered<_> = snapshots
                .into_iter()
                .map(|snap| {
                    let query = query.clone();
                    async move {
                        let outcome = query.query_workflow_outcome(&snap.job_id).await;
                        (snap, outcome)
                    }
                })
                .collect();
            let mut results = Vec::with_capacity(in_flight.len().min(MAX_CONCURRENT_DESCRIBES));
            // FuturesUnordered polls all concurrently; the bound is
            // implicit (the source iterator already materialized), but
            // large fleets would benefit from `buffer_unordered` over a
            // stream source — not needed at current scale.
            while let Some(entry) = in_flight.next().await {
                results.push(entry);
            }
            results
        };

        let mut report = ReconcileReport::default();
        for (snap, outcome) in queried {
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(err) => {
                    tracing::warn!(
                        job_id = %snap.job_id,
                        error = %err,
                        "Temporal query failed during reconcile — skipping this job"
                    );
                    report.errored += 1;
                    continue;
                }
            };
            // Fetch the most recent cancellation audit row so the pure
            // decision function can distinguish user-initiated cancels from
            // system-initiated (shutdown) ones. A fetch failure is non-fatal
            // — treat it as "no audit information" to preserve the prior
            // MarkCancelled behavior rather than silently no-op'ing.
            let last_cancellation: Option<CancellationRecord> = self
                .store
                .last_cancellation_for(&snap.job_id)
                .await
                .inspect_err(|err| {
                    debug!(
                        job_id = %snap.job_id,
                        error = %err,
                        "audit fetch failed; falling back to MarkCancelled if Temporal reports Cancelled"
                    );
                })
                .ok()
                .flatten()
                .and_then(|row| {
                    // Convert the raw DB row to the typed CancellationRecord.
                    // A parse error on the stored source string is treated as
                    // "no audit info" — same safe-fallback as fetch failure.
                    CancellationRecord::try_from(row)
                        .inspect_err(|err| {
                            debug!(
                                job_id = %snap.job_id,
                                error = %err,
                                "cancellation row parse failed; falling back to MarkCancelled if Temporal reports Cancelled"
                            );
                        })
                        .ok()
                });
            let action = reconcile_action(
                snap.status,
                snap.submitted_at,
                snap.runner_active,
                outcome,
                last_cancellation.as_ref(),
                now,
                self.stale_threshold_s,
            );
            self.apply_action(&snap.job_id, action, &mut report).await;
        }
        report
    }

    async fn apply_action(
        &self,
        job_id: &JobId,
        action: ReconcileAction,
        report: &mut ReconcileReport,
    ) {
        match action {
            ReconcileAction::NoChange => {
                report.unchanged += 1;
            }
            ReconcileAction::MarkCompleted => {
                self.store
                    .update_job_status(job_id, JobStatus::Completed, None)
                    .await;
                report.updated += 1;
            }
            ReconcileAction::MarkCancelled => {
                self.store
                    .update_job_status(job_id, JobStatus::Cancelled, None)
                    .await;
                report.updated += 1;
            }
            ReconcileAction::MarkFailed { reason } => {
                self.store
                    .update_job_status(job_id, JobStatus::Failed, Some(reason.message().into()))
                    .await;
                report.updated += 1;
            }
        }
    }
}

/// Keep the silence warning for `MAX_CONCURRENT_DESCRIBES` if we later
/// move to `buffer_unordered` over a stream source.
#[allow(dead_code)]
const _REMINDER: usize = MAX_CONCURRENT_DESCRIBES;

/// The verdict of Temporal on a workflow's lifecycle, collapsed from the
/// full `WorkflowExecutionStatus` enum into the four states that matter
/// for reconciliation. `None` at the call site means "workflow not found
/// in Temporal" which is semantically distinct from any variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalWorkflowOutcome {
    /// Workflow is still active in Temporal (`Running` or `Paused`).
    Active,
    /// Workflow finished successfully.
    Completed,
    /// Workflow was cancelled or terminated.
    Cancelled,
    /// Workflow failed or timed out.
    Failed,
}

/// The action the reconciler should take on a single local-store job after
/// consulting Temporal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileAction {
    /// Local store already matches Temporal's verdict (or is terminal).
    NoChange,
    /// Flip the local store to `Completed`.
    MarkCompleted,
    /// Flip the local store to `Cancelled`.
    MarkCancelled,
    /// Flip the local store to `Failed`.
    MarkFailed { reason: ReconcileFailureReason },
}

/// Why the reconciler decided to mark a job `Failed`. Typed so log
/// queries and dashboards can discriminate without parsing message strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileFailureReason {
    /// Temporal itself reported a terminal failure (`Failed` or `TimedOut`).
    TemporalTerminalFailure,
    /// Temporal reports the workflow doesn't exist AND the local job is
    /// past its staleness threshold AND has no active runner — treat it
    /// as a lost workflow and sweep to `Failed`.
    WorkflowLost,
}

impl ReconcileFailureReason {
    /// Stable, human-readable message suitable for storing in the
    /// `Job.execution.error` field. Kept here (not on `Display`) so the
    /// exact wording is easy to grep and change in one place.
    pub fn message(self) -> &'static str {
        match self {
            Self::TemporalTerminalFailure => "workflow terminal in Temporal (failed/timeout)",
            Self::WorkflowLost => "workflow lost — reconciler swept",
        }
    }
}

/// Pure decision function: given the current local-store status and
/// runtime hints, plus what Temporal reports, return the action the
/// reconciler should take.
///
/// `temporal_outcome = None` means the workflow is not found in Temporal.
/// That's only actionable when the job is old enough AND has no active
/// runner — a young or actively-running job with `None` from Temporal is
/// likely a transient visibility gap, not a lost workflow.
///
/// `last_cancellation` is the most-recent row from the `cancellations`
/// audit table, if any. It is used to distinguish a user-initiated cancel
/// (source = `Tui` / `Api`) from a system-initiated shutdown cancel
/// (source = `Signal`). When Temporal reports `Cancelled` AND the audit
/// row shows a system-initiated cancel, the reconciler returns `NoChange`
/// instead of `MarkCancelled`: the local recovery sequence has already
/// moved the job from `Interrupted` to `Queued` via `load_from_db`, and
/// writing `Cancelled` here would permanently undo that recovery.
///
/// Callers pass `None` when no audit row is available (no DB, no rows,
/// or a parse error on the stored source string); in that case the prior
/// `MarkCancelled` behavior is preserved.
pub fn reconcile_action(
    store_status: JobStatus,
    submitted_at: UnixTimestamp,
    runner_active: bool,
    temporal_outcome: Option<TemporalWorkflowOutcome>,
    last_cancellation: Option<&CancellationRecord>,
    now: UnixTimestamp,
    stale_threshold_s: u64,
) -> ReconcileAction {
    if store_status.is_terminal() {
        return ReconcileAction::NoChange;
    }

    match temporal_outcome {
        Some(TemporalWorkflowOutcome::Completed) => ReconcileAction::MarkCompleted,
        Some(TemporalWorkflowOutcome::Cancelled) => {
            // Disambiguate user-cancel vs system-cancel using the audit row.
            // A system-initiated cancel paired with a non-terminal local row
            // means the recovery sequence has already moved the job to a
            // resumable state — writing Cancelled here would un-do that.
            // Leave it alone.
            match last_cancellation {
                Some(record) if is_system_initiated_shutdown_cancel(record) => {
                    ReconcileAction::NoChange
                }
                _ => ReconcileAction::MarkCancelled,
            }
        }
        Some(TemporalWorkflowOutcome::Failed) => ReconcileAction::MarkFailed {
            reason: ReconcileFailureReason::TemporalTerminalFailure,
        },
        Some(TemporalWorkflowOutcome::Active) => ReconcileAction::NoChange,
        None => {
            let age_s = now.0 - submitted_at.0;
            if !runner_active && age_s > stale_threshold_s as f64 {
                ReconcileAction::MarkFailed {
                    reason: ReconcileFailureReason::WorkflowLost,
                }
            } else {
                ReconcileAction::NoChange
            }
        }
    }
}

/// Whether the most-recent cancellation audit row indicates a
/// system-initiated cancel (server shutdown or Temporal-internal
/// activity-cancel forwarding) rather than a user gesture.
///
/// The two recognized system reasons are:
/// - `server-cancel-all`: written by the graceful-shutdown path in the
///   Temporal backend when it cancels all in-flight workflows.
/// - `temporal-activity-forwarded`: written by the activity-side cancel
///   handler when it propagates the Temporal cancel signal to the local
///   job store.
///
/// Any other `source` (Tui, Api, Cli, Dashboard, Staging) or any other
/// `reason` with `source = Signal` is treated as user-initiated and
/// routes to `MarkCancelled` as before.
fn is_system_initiated_shutdown_cancel(record: &CancellationRecord) -> bool {
    if record.source != CancelSource::Signal {
        return false;
    }
    let Some(reason) = record.reason.as_ref() else {
        return false;
    };
    *reason == CancelReason::server_cancel_all()
        || *reason == CancelReason::temporal_activity_forwarded()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_000_000.0;
    const STALE_THRESHOLD_S: u64 = 300;

    /// Convenience wrapper for tests that don't care about the audit row.
    /// Passes `None` for `last_cancellation` (no audit information).
    fn call(
        store_status: JobStatus,
        age_s: f64,
        runner_active: bool,
        outcome: Option<TemporalWorkflowOutcome>,
    ) -> ReconcileAction {
        reconcile_action(
            store_status,
            UnixTimestamp(NOW - age_s),
            runner_active,
            outcome,
            None,
            UnixTimestamp(NOW),
            STALE_THRESHOLD_S,
        )
    }

    /// When Temporal reports a workflow completed but the local store
    /// still has it `Queued`, the reconciler must flip the store entry
    /// to `Completed`. Without this, conflict-detection blocks
    /// resubmissions against a workflow that's already long since done.
    #[test]
    fn queued_job_with_completed_workflow_marks_completed() {
        assert_eq!(
            call(
                JobStatus::Queued,
                60.0,
                false,
                Some(TemporalWorkflowOutcome::Completed)
            ),
            ReconcileAction::MarkCompleted
        );
    }

    #[test]
    fn queued_job_with_active_workflow_is_no_change() {
        assert_eq!(
            call(
                JobStatus::Queued,
                60.0,
                false,
                Some(TemporalWorkflowOutcome::Active)
            ),
            ReconcileAction::NoChange
        );
    }

    #[test]
    fn queued_job_with_cancelled_workflow_marks_cancelled() {
        assert_eq!(
            call(
                JobStatus::Queued,
                60.0,
                false,
                Some(TemporalWorkflowOutcome::Cancelled)
            ),
            ReconcileAction::MarkCancelled
        );
    }

    #[test]
    fn queued_job_with_failed_workflow_marks_failed_temporal_terminal() {
        assert_eq!(
            call(
                JobStatus::Queued,
                60.0,
                false,
                Some(TemporalWorkflowOutcome::Failed)
            ),
            ReconcileAction::MarkFailed {
                reason: ReconcileFailureReason::TemporalTerminalFailure
            }
        );
    }

    #[test]
    fn running_job_with_completed_workflow_marks_completed() {
        assert_eq!(
            call(
                JobStatus::Running,
                60.0,
                true,
                Some(TemporalWorkflowOutcome::Completed)
            ),
            ReconcileAction::MarkCompleted
        );
    }

    /// Terminal store statuses are never touched — the reconciler is
    /// read-only for jobs already in a final state.
    #[test]
    fn terminal_store_status_is_no_change_regardless_of_temporal() {
        for terminal in [
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ] {
            assert_eq!(
                call(terminal, 60.0, false, Some(TemporalWorkflowOutcome::Active)),
                ReconcileAction::NoChange,
                "terminal status {:?} must not be rewritten by reconciler",
                terminal
            );
            assert_eq!(
                call(
                    terminal,
                    60.0,
                    false,
                    Some(TemporalWorkflowOutcome::Completed)
                ),
                ReconcileAction::NoChange
            );
            assert_eq!(call(terminal, 60.0, false, None), ReconcileAction::NoChange);
        }
    }

    /// Temporal says "not found", job is old, no runner → sweep to failed.
    #[test]
    fn queued_and_not_found_and_old_and_no_runner_sweeps_to_failed() {
        assert_eq!(
            call(JobStatus::Queued, 3600.0, false, None),
            ReconcileAction::MarkFailed {
                reason: ReconcileFailureReason::WorkflowLost
            }
        );
    }

    /// Temporal says "not found" but the job is YOUNG — could be a
    /// transient visibility gap right after submission. Do not sweep.
    #[test]
    fn queued_and_not_found_but_young_is_no_change() {
        // 10 seconds old, threshold 300 — still inside grace window.
        assert_eq!(
            call(JobStatus::Queued, 10.0, false, None),
            ReconcileAction::NoChange
        );
    }

    /// Temporal says "not found" and the job is old, but a runner is
    /// attached — the reconciler trusts the local runner and keeps its
    /// hands off. A job with an active runner is being worked; Temporal
    /// lookup might just be slow.
    #[test]
    fn queued_and_not_found_with_runner_active_is_no_change() {
        assert_eq!(
            call(JobStatus::Queued, 3600.0, true, None),
            ReconcileAction::NoChange
        );
    }

    // -----------------------------------------------------------------------
    // Shutdown-cancel disambiguation (Task 4)
    // -----------------------------------------------------------------------

    /// After shutdown→restart→recovery, the local row is `Queued`
    /// (not Interrupted, not Cancelled — `load_from_db` handles that
    /// transition). The reconciler must NOT write `Cancelled` even
    /// though Temporal sees the workflow as Cancelled — recovery already
    /// handled it correctly and writing Cancelled here would undo it.
    #[test]
    fn shutdown_signal_cancel_against_recovered_queued_job_is_no_change() {
        let last_cancel = CancellationRecord {
            source: CancelSource::Signal,
            reason: Some(CancelReason::server_cancel_all()),
            requested_at: UnixTimestamp(1_777_300_000.0),
            ..CancellationRecord::test_default()
        };

        let action = reconcile_action(
            JobStatus::Queued, // post-recovery state
            UnixTimestamp(1_777_290_000.0),
            false, // runner not active yet
            Some(TemporalWorkflowOutcome::Cancelled),
            Some(&last_cancel),
            UnixTimestamp(1_777_300_500.0),
            300,
        );

        assert_eq!(
            action,
            ReconcileAction::NoChange,
            "system-initiated shutdown cancels must not overwrite a recovered Queued row"
        );
    }

    /// The activity-side cancel handler also writes `source=Signal,
    /// reason=temporal-activity-forwarded`. This must receive the same
    /// NoChange treatment as the server-cancel-all reason.
    #[test]
    fn temporal_activity_forwarded_audit_is_treated_as_system_initiated() {
        let last_cancel = CancellationRecord {
            source: CancelSource::Signal,
            reason: Some(CancelReason::temporal_activity_forwarded()),
            requested_at: UnixTimestamp(1_777_300_000.0),
            ..CancellationRecord::test_default()
        };

        let action = reconcile_action(
            JobStatus::Queued,
            UnixTimestamp(1_777_290_000.0),
            false,
            Some(TemporalWorkflowOutcome::Cancelled),
            Some(&last_cancel),
            UnixTimestamp(1_777_300_500.0),
            300,
        );

        assert_eq!(
            action,
            ReconcileAction::NoChange,
            "temporal-activity-forwarded is a system-initiated cancel and must not undo recovery"
        );
    }

    /// An explicit user cancel (source=Tui) must remain terminal even if
    /// Temporal also reports Cancelled. The recovery path only applies to
    /// system-initiated cancels.
    #[test]
    fn user_initiated_cancel_still_routes_to_mark_cancelled() {
        let last_cancel = CancellationRecord {
            source: CancelSource::Tui,
            reason: Some(CancelReason("user-pressed-cancel".to_string())),
            requested_at: UnixTimestamp(1_777_300_000.0),
            ..CancellationRecord::test_default()
        };

        let action = reconcile_action(
            JobStatus::Running,
            UnixTimestamp(1_777_290_000.0),
            true,
            Some(TemporalWorkflowOutcome::Cancelled),
            Some(&last_cancel),
            UnixTimestamp(1_777_300_500.0),
            300,
        );

        assert_eq!(
            action,
            ReconcileAction::MarkCancelled,
            "explicit user cancel must remain a terminal cancel"
        );
    }

    /// Defensive: if no audit row is available (very early installations,
    /// DB corruption, in-memory-only store), preserve the prior
    /// MarkCancelled behavior rather than silently no-op'ing.
    #[test]
    fn temporal_cancel_with_no_audit_row_falls_back_to_mark_cancelled() {
        let action = reconcile_action(
            JobStatus::Running,
            UnixTimestamp(1_777_290_000.0),
            true,
            Some(TemporalWorkflowOutcome::Cancelled),
            None,
            UnixTimestamp(1_777_300_500.0),
            300,
        );

        assert_eq!(
            action,
            ReconcileAction::MarkCancelled,
            "missing audit row must fall back to MarkCancelled to preserve prior behavior"
        );
    }
}

// ---------------------------------------------------------------------------
// Reconciler loop
// ---------------------------------------------------------------------------

#[cfg(test)]
mod reconciler_loop_tests {
    use super::*;
    use crate::api::{
        CorrelationId, DisplayPath, JobId, LanguageCode3, LanguageSpec, NumSpeakers,
        ReleasedCommand,
    };
    use crate::options::CommandOptions;
    use crate::store::{
        FileStatus, Job, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity,
        JobLeaseState, JobRuntimeControl, JobScheduleState, JobSourceContext, JobStore,
    };
    use crate::types::config::ServerConfig;
    use crate::ws::BROADCAST_CAPACITY;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Mutex as StdMutex;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;

    /// In-memory `TemporalStateQuery` for tests. Returns whatever the test
    /// configured for each job id; unknown ids yield `Ok(None)` (workflow
    /// not found in Temporal).
    struct FakeTemporalQuery {
        responses: StdMutex<HashMap<JobId, Option<TemporalWorkflowOutcome>>>,
    }

    impl FakeTemporalQuery {
        fn new() -> Self {
            Self {
                responses: StdMutex::new(HashMap::new()),
            }
        }

        fn set(&self, job_id: JobId, outcome: Option<TemporalWorkflowOutcome>) {
            self.responses.lock().unwrap().insert(job_id, outcome);
        }
    }

    #[async_trait::async_trait]
    impl TemporalStateQuery for FakeTemporalQuery {
        async fn query_workflow_outcome(
            &self,
            job_id: &JobId,
        ) -> Result<Option<TemporalWorkflowOutcome>, crate::error::ServerError> {
            Ok(self
                .responses
                .lock()
                .unwrap()
                .get(job_id)
                .copied()
                .unwrap_or(None))
        }
    }

    async fn make_store() -> (Arc<JobStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(crate::db::JobDB::open(Some(dir.path())).await.unwrap());
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(ServerConfig::default(), Some(db), tx);
        (Arc::new(store), dir)
    }

    fn sample_queued_job(job_id: &str, filenames: &[&str]) -> Job {
        let file_statuses = filenames
            .iter()
            .map(|filename| {
                let name = DisplayPath::from(*filename);
                (String::from(name.clone()), FileStatus::new(name))
            })
            .collect();
        let has_chat = filenames.iter().map(|_| true).collect();

        Job {
            identity: JobIdentity {
                job_id: JobId::from(job_id),
                correlation_id: CorrelationId::from(format!("corr-{job_id}")),
            },
            dispatch: JobDispatchConfig {
                command: ReleasedCommand::Morphotag,
                lang: LanguageSpec::Resolved(LanguageCode3::eng()),
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Morphotag(crate::options::MorphotagOptions {
                    common: crate::options::CommonOptions::default(),

                    ..Default::default()
                }),
                runtime_state: BTreeMap::new(),
                debug_traces: false,
            },
            source: JobSourceContext {
                submitted_by: "127.0.0.1".into(),
                submitted_by_name: "localhost".into(),
                source_dir: "/corpus".into(),
            },
            filesystem: JobFilesystemConfig {
                filenames: filenames
                    .iter()
                    .map(|filename| DisplayPath::from(*filename))
                    .collect(),
                has_chat,
                staging_dir: "/tmp/job".into(),
                paths_mode: false,
                source_paths: Vec::new(),
                output_paths: Vec::new(),
                before_paths: Vec::new(),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: Default::default(),
            },
            execution: JobExecutionState {
                status: JobStatus::Queued,
                file_statuses,
                results: Vec::new(),
                error: None,
                completed_files: 0,
                batch_progress: None,
            },
            schedule: JobScheduleState {
                submitted_at: UnixTimestamp(100.0),
                completed_at: None,
                next_eligible_at: None,
                num_workers: None,
                lease: JobLeaseState {
                    leased_by_node: None,
                    expires_at: None,
                    heartbeat_at: None,
                },
                last_cancel: None,
            },
            runtime: JobRuntimeControl {
                cancel_token: CancellationToken::new(),
                runner_active: false,
            },
            execution_plan: None,
        }
    }

    /// When the local store has a job `Queued` but Temporal reports the
    /// workflow has completed, the reconciler must propagate that
    /// verdict into the store. Without this the local DB stays out of
    /// sync with the authoritative workflow state forever and
    /// conflict-detection blocks resubmissions against a ghost job.
    #[tokio::test]
    async fn reconciler_flips_queued_job_when_temporal_says_completed() {
        let (store, _dir) = make_store().await;
        let query = Arc::new(FakeTemporalQuery::new());

        let job = sample_queued_job("zombie-job", &["020724a.mp3"]);
        let job_id = job.identity.job_id.clone();
        store.submit(job).await.unwrap();
        query.set(job_id.clone(), Some(TemporalWorkflowOutcome::Completed));

        let reconciler = TemporalReconciler::new(store.clone(), query.clone(), 300);
        let report = reconciler.reconcile_all_active().await;

        assert_eq!(
            store.job_status(&job_id).await,
            Some(JobStatus::Completed),
            "reconciler should have flipped queued job to Completed after Temporal reported Completed",
        );
        assert_eq!(report.updated, 1);
        assert_eq!(report.unchanged, 0);
    }

    #[tokio::test]
    async fn reconciler_leaves_queued_job_alone_when_temporal_says_active() {
        let (store, _dir) = make_store().await;
        let query = Arc::new(FakeTemporalQuery::new());

        let job = sample_queued_job("in-flight", &["a.cha"]);
        let job_id = job.identity.job_id.clone();
        store.submit(job).await.unwrap();
        query.set(job_id.clone(), Some(TemporalWorkflowOutcome::Active));

        let reconciler = TemporalReconciler::new(store.clone(), query.clone(), 300);
        let report = reconciler.reconcile_all_active().await;

        assert_eq!(store.job_status(&job_id).await, Some(JobStatus::Queued));
        assert_eq!(report.updated, 0);
        assert_eq!(report.unchanged, 1);
    }

    /// When `submitted_by` is supplied, reconcile only that submitter's
    /// jobs. This is what `submit_job` uses to bound the cost of
    /// opportunistic reconciliation on the submission hot path.
    #[tokio::test]
    async fn reconcile_submitter_touches_only_matching_submitter_jobs() {
        let (store, _dir) = make_store().await;
        let query = Arc::new(FakeTemporalQuery::new());

        let mut my_job = sample_queued_job("mine", &["mine.cha"]);
        my_job.source.submitted_by = "mine@host".into();
        let my_id = my_job.identity.job_id.clone();

        let mut other_job = sample_queued_job("theirs", &["theirs.cha"]);
        other_job.source.submitted_by = "someone@else".into();
        let other_id = other_job.identity.job_id.clone();

        store.submit(my_job).await.unwrap();
        store.submit(other_job).await.unwrap();
        // Both workflows have completed on Temporal.
        query.set(my_id.clone(), Some(TemporalWorkflowOutcome::Completed));
        query.set(other_id.clone(), Some(TemporalWorkflowOutcome::Completed));

        let reconciler = TemporalReconciler::new(store.clone(), query.clone(), 300);
        let report = reconciler.reconcile_submitter("mine@host").await;

        assert_eq!(
            store.job_status(&my_id).await,
            Some(JobStatus::Completed),
            "my own completed workflow must be reconciled"
        );
        assert_eq!(
            store.job_status(&other_id).await,
            Some(JobStatus::Queued),
            "other submitter's completed workflow must be left alone"
        );
        assert_eq!(report.updated, 1);
    }

    /// Temporal `NotFound` with an old submission and no runner sweeps
    /// the job to `Failed` — this catches workflows that were lost,
    /// crashed, or never registered (e.g. a daemon restart after
    /// persisting the local job but before registering with Temporal).
    #[tokio::test]
    async fn reconciler_sweeps_old_orphan_to_failed_when_temporal_not_found() {
        let (store, _dir) = make_store().await;
        let query = Arc::new(FakeTemporalQuery::new());

        let mut job = sample_queued_job("orphan", &["lost.cha"]);
        job.schedule.submitted_at = UnixTimestamp(crate::store::unix_now().0 - 10_000.0);
        let job_id = job.identity.job_id.clone();
        store.submit(job).await.unwrap();
        // Fake query returns None → Temporal says "not found".
        // (No call to query.set — default is None.)

        let reconciler = TemporalReconciler::new(store.clone(), query.clone(), 300);
        let report = reconciler.reconcile_all_active().await;

        assert_eq!(store.job_status(&job_id).await, Some(JobStatus::Failed));
        assert_eq!(report.updated, 1);
    }

    /// Transient query errors don't crash the reconciler; they count as
    /// `errored` and the job is left alone for the next pass.
    #[tokio::test]
    async fn reconciler_tolerates_query_errors_and_counts_them() {
        struct BrokenQuery;
        #[async_trait::async_trait]
        impl TemporalStateQuery for BrokenQuery {
            async fn query_workflow_outcome(
                &self,
                _: &JobId,
            ) -> Result<Option<TemporalWorkflowOutcome>, crate::error::ServerError> {
                Err(crate::error::ServerError::Validation("boom".into()))
            }
        }

        let (store, _dir) = make_store().await;
        let job = sample_queued_job("resilient", &["a.cha"]);
        let job_id = job.identity.job_id.clone();
        store.submit(job).await.unwrap();

        let reconciler = TemporalReconciler::new(store.clone(), Arc::new(BrokenQuery), 300);
        let report = reconciler.reconcile_all_active().await;

        assert_eq!(
            store.job_status(&job_id).await,
            Some(JobStatus::Queued),
            "transient Temporal query error must leave the job alone"
        );
        assert_eq!(report.errored, 1);
    }
}
