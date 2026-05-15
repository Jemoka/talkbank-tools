//! Query, mutation, and notification methods on [`JobStore`].

mod db_helpers;
mod dispatch;
mod execution;
pub(crate) mod file_state;
mod lifecycle;
mod recovery;
mod runner;

pub(crate) use db_helpers::{
    AttemptFinishRecord, AttemptStartRecord, PersistedFileUpdate, PersistedJobUpdate,
};
pub(crate) use dispatch::LeaseRenewalOutcome;

use crate::api::{CancellationRequest, JobId, JobInfo, JobListItem, JobStatus, UnixTimestamp};

/// Snapshot of a job's reconciliation-relevant state, used by the
/// Temporal reconciler to decide whether to mutate the local store.
/// See `crate::temporal_reconciler::reconcile_action`.
#[derive(Debug, Clone)]
pub(crate) struct ReconcilableJobSnapshot {
    pub job_id: JobId,
    pub status: JobStatus,
    pub submitted_at: UnixTimestamp,
    pub runner_active: bool,
}
use tracing::warn;

use super::{JobDetail, JobStore, OperationalCounters, unix_now};
use crate::error::ServerError;
use crate::ws::WsEvent;

/// Pass through a `Some(&str)` only when the contained string is non-empty.
/// Used at the cancellation-audit boundary so the DB sees genuine NULLs
/// instead of empty strings; lets `string_id!` newtypes (which still
/// permit empties at construction) project cleanly into nullable columns.
fn non_empty(s: Option<&str>) -> Option<&str> {
    s.filter(|v| !v.is_empty())
}

impl JobStore {
    /// Seconds before a locally-dispatched file lease is considered orphaned.
    /// Reads from `ServerConfig::local_lease_ttl_s` at construction; this
    /// method provides a convenience accessor for query code.
    fn local_lease_ttl_s(&self) -> f64 {
        self.config().local_lease_ttl_s as f64
    }
    pub(crate) const LOCAL_LEASE_HEARTBEAT_S: u64 = 60;

    /// Look up a job by ID.
    pub async fn get(&self, job_id: &JobId) -> Option<JobInfo> {
        self.registry.job_info(job_id).await
    }

    /// Snapshot every non-terminal job's status + liveness signals for
    /// Temporal reconciliation.
    ///
    /// Returns the minimum fields `temporal_reconciler::reconcile_action`
    /// needs: job id, current store status, submission timestamp, and
    /// whether a runner is currently attached. Only the non-terminal
    /// jobs are included — terminal ones never need reconciliation.
    ///
    /// Optionally filter by `submitted_by` to produce a narrow snapshot
    /// for opportunistic reconciliation on a new submission (we only
    /// need to check jobs from the same submitter for conflict-detection
    /// freshness).
    pub(crate) async fn reconcilable_snapshot(
        &self,
        submitted_by_filter: Option<String>,
    ) -> Vec<ReconcilableJobSnapshot> {
        self.registry
            .inspect_all(move |jobs| {
                jobs.values()
                    .filter(|job| !job.execution.status.is_terminal())
                    .filter(|job| match submitted_by_filter.as_deref() {
                        Some(sb) => job.source.submitted_by == sb,
                        None => true,
                    })
                    .map(|job| ReconcilableJobSnapshot {
                        job_id: job.identity.job_id.clone(),
                        status: job.execution.status,
                        submitted_at: job.schedule.submitted_at,
                        runner_active: job.runtime.runner_active,
                    })
                    .collect()
            })
            .await
    }

    /// Return all jobs (newest first).
    pub async fn list_all(&self) -> Vec<JobListItem> {
        self.registry.list_items().await
    }

    /// Signal a job to cancel, recording who/why/when in the
    /// `cancellations` audit table and projecting the most-recent
    /// metadata onto `jobs.last_cancelled_*` columns.
    ///
    /// `provenance` is the caller's self-report from the
    /// `POST /jobs/{id}/cancel` body (TUI fills it; raw curl leaves
    /// it default). Audit row is persisted regardless of whether the
    /// cancel actually changed job state — `accepted=false` records
    /// "user pressed cancel against an already-finished job," which
    /// is itself diagnostic.
    pub async fn cancel(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError> {
        let now = unix_now();
        let registry_outcome = self.registry.request_cancellation(job_id, now).await;
        let accepted = registry_outcome.is_some();

        // Record the audit row regardless of whether the cancel mutated
        // state — `accepted=false` distinguishes "user pressed cancel
        // against an already-finished job" from a state-changing cancel.
        self.record_audit_row(job_id, &provenance, now, accepted)
            .await;

        if !accepted {
            return Err(ServerError::JobNotFound(job_id.clone()));
        }

        // Persist Cancelled status even if the runner is stuck in
        // synchronous code and hasn't seen the in-memory cancellation
        // token yet — otherwise a daemon restart resurrects the job.
        let completed_at = registry_outcome.and_then(|inner| inner).unwrap_or(now);
        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: JobStatus::Cancelled,
                error: None,
                completed_at: Some(completed_at),
                num_workers: None,
                next_eligible_at: None,
            },
        )
        .await;
        Ok(())
    }

    /// Record a cancel-attempt audit row WITHOUT changing job state.
    /// Used by the route handler when a cancel arrives against a job that
    /// is already terminal.
    pub async fn record_terminal_cancel(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError> {
        let now = unix_now();
        self.record_audit_row(job_id, &provenance, now, false).await;
        Ok(())
    }

    /// Persist one cancel attempt to the audit table and project the
    /// most-recent metadata onto the in-memory `Job` so `JobInfo`'s
    /// `last_cancelled_*` fields reflect this cancel without a DB JOIN.
    /// Both `cancel` and `record_terminal_cancel` flow through here.
    ///
    /// Failures to write the audit row are logged at WARN but do not
    /// propagate — the caller's primary cancel work (state change) still
    /// runs even if the audit write fails. Forensic rows are best-effort,
    /// not load-bearing on cancel correctness.
    async fn record_audit_row(
        &self,
        job_id: &JobId,
        provenance: &CancellationRequest,
        requested_at: UnixTimestamp,
        accepted: bool,
    ) {
        let source = provenance.source.unwrap_or(crate::api::CancelSource::Api);
        let source_str = source.to_string();
        let host_str = non_empty(provenance.host.as_ref().map(AsRef::as_ref));
        let reason_str = non_empty(provenance.reason.as_ref().map(AsRef::as_ref));
        let correlation_str = non_empty(provenance.correlation_id.as_ref().map(AsRef::as_ref));
        let in_flight_str = non_empty(provenance.in_flight_filename.as_ref().map(AsRef::as_ref));
        let pid_value = provenance.pid.map(|p| p.0);

        if let Some(db) = &self.db
            && let Err(e) = db
                .insert_cancellation(
                    job_id,
                    requested_at.0,
                    &source_str,
                    host_str,
                    pid_value,
                    reason_str,
                    correlation_str,
                    in_flight_str,
                    accepted,
                )
                .await
        {
            tracing::warn!(
                job_id = %job_id,
                error = %e,
                "DB insert_cancellation failed"
            );
        }

        let info = crate::store::JobLastCancelInfo {
            at: requested_at,
            source: source_str,
            host: host_str.map(str::to_owned),
            reason: reason_str.map(str::to_owned),
        };
        self.registry.set_last_cancel(job_id, info).await;
    }

    /// Fetch the most recent cancellation audit row for a job, if any.
    ///
    /// Delegates to `JobDB::last_cancellation_for`. Returns `Ok(None)` when
    /// there is no DB (in-memory-only store) or when the job has no audit
    /// rows, so callers can treat `None` uniformly as "no audit information."
    ///
    /// See `JobDB::last_cancellation_for` for the reconciler use-case that
    /// motivates this helper.
    pub async fn last_cancellation_for(
        &self,
        job_id: &JobId,
    ) -> Result<Option<crate::db::CancellationRow>, ServerError> {
        match &self.db {
            Some(db) => db.last_cancellation_for(job_id).await,
            None => Ok(None),
        }
    }

    /// Read every cancel-attempt row for a job. Returns plain rows; the
    /// route layer maps them to wire-format `CancellationRecord`.
    pub async fn list_cancellations(
        &self,
        job_id: &JobId,
    ) -> Result<Vec<crate::db::CancellationRow>, ServerError> {
        match &self.db {
            Some(db) => db.list_cancellations(job_id).await,
            None => Ok(Vec::new()),
        }
    }

    /// Remove a job from the store.
    pub async fn delete(&self, job_id: &JobId) -> Result<(), ServerError> {
        let staging_dir = self
            .registry
            .remove_staging_dir(job_id)
            .await
            .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))?;

        // Clean up staged content after releasing the jobs lock.
        if !staging_dir.as_str().is_empty() {
            let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        }

        if let Some(db) = &self.db
            && let Err(e) = db.delete_job(job_id).await
        {
            warn!(job_id = %job_id, error = %e, "Failed to delete job from DB");
        }

        let _ = self.ws_tx.send(WsEvent::JobDeleted {
            job_id: job_id.clone(),
        });
        Ok(())
    }

    /// Check if a job is running (for delete guard).
    pub async fn is_running(&self, job_id: &JobId) -> Option<bool> {
        self.registry.is_running(job_id).await
    }

    /// Get the status of a specific job.
    pub async fn job_status(&self, job_id: &JobId) -> Option<JobStatus> {
        self.registry.job_status(job_id).await
    }

    /// Set the execution plan for a job (used by staged remote orchestrator).
    pub async fn set_execution_plan(
        &self,
        job_id: &JobId,
        plan: Option<crate::types::execution_plan::ExecutionPlan>,
    ) {
        self.registry
            .update_job(job_id.clone(), move |job| {
                job.execution_plan = plan;
            })
            .await;
    }

    /// Update a job's status and optional error message.
    ///
    /// Used by the staged remote orchestrator to set terminal states
    /// (`Completed`, `Failed`, `WritebackFailed`) after remote execution.
    pub async fn update_job_status(
        &self,
        job_id: &JobId,
        status: JobStatus,
        error: Option<String>,
    ) {
        let error_clone = error.clone();
        let completed_at = if status.is_terminal() {
            Some(super::unix_now())
        } else {
            None
        };
        self.registry
            .update_job(job_id.clone(), move |job| {
                job.execution.status = status;
                if let Some(err) = error_clone {
                    job.execution.error = Some(err);
                }
                if let Some(ts) = completed_at {
                    job.schedule.completed_at = Some(ts);
                }
            })
            .await;

        // Persist to SQLite
        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status,
                error: error.as_deref(),
                completed_at,
                num_workers: None,
                next_eligible_at: None,
            },
        )
        .await;
    }

    /// Count of currently running jobs.
    pub async fn active_jobs(&self) -> i64 {
        self.registry.active_jobs().await
    }

    /// Approximate number of job slots available.
    pub async fn workers_available(&self) -> i64 {
        let active = self.active_jobs().await;
        (self.max_concurrent as i64 - active).max(0)
    }

    /// Operational counters for health endpoint.
    pub async fn operational_counters(&self) -> (i64, i64, i64, i64, i64, i64) {
        self.counters
            .inspect(|counters| {
                (
                    counters.worker_crashes,
                    counters.attempts_started,
                    counters.attempts_retried,
                    counters.deferred_work_units,
                    counters.forced_terminal_errors,
                    counters.memory_gate_aborts,
                )
            })
            .await
    }

    pub(crate) async fn bump_counter(&self, f: impl FnOnce(&mut OperationalCounters)) {
        self.counters.mutate(f).await;
    }

    /// Get the staging dir for a job (used by results endpoint).
    pub async fn get_job_detail(&self, job_id: &JobId) -> Option<JobDetail> {
        self.registry.job_detail(job_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use crate::api::{JobStatus, ReleasedCommand};
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::api::{DisplayPath, UnixTimestamp};
    use crate::db::JobDB;
    use crate::options::FaEngineName;
    use crate::store::job::{
        Job, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity, JobLeaseState,
        JobRuntimeControl, JobScheduleState, JobSourceContext,
    };
    use crate::store::{FileStatus, auto_max_concurrent_from, ts_iso, unix_now};
    use crate::ws::BROADCAST_CAPACITY;

    pub(super) fn test_config() -> crate::config::ServerConfig {
        crate::config::ServerConfig {
            max_concurrent_jobs: Some(2),
            ..Default::default()
        }
    }

    /// Create a store backed by a temporary SQLite database.
    async fn test_store_with_db() -> (JobStore, Arc<JobDB>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(JobDB::open(Some(dir.path())).await.unwrap());
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), Some(db.clone()), tx);
        (store, db, dir)
    }

    pub(super) fn make_job(
        id: &str,
        command: crate::api::ReleasedCommand,
        filenames: Vec<String>,
    ) -> Job {
        use crate::options::{AlignOptions, CommandOptions, CommonOptions, MorphotagOptions};

        let mut file_statuses = HashMap::new();
        let has_chat: Vec<bool> = filenames.iter().map(|_| true).collect();
        for f in &filenames {
            file_statuses.insert(
                f.clone(),
                FileStatus::new(crate::api::DisplayPath::from(f.as_str())),
            );
        }

        let options = match command {
            crate::api::ReleasedCommand::Align => CommandOptions::Align(AlignOptions {
                common: CommonOptions::default(),
                fa_engine: FaEngineName::Wave2Vec,
                utr_engine: None,
                utr_overlap_strategy: Default::default(),
                utr_two_pass: Default::default(),
                pauses: false,
                wor: true.into(),
                merge_abbrev: false.into(),
                media_dir: None,
                bullet_repair: false,
                review_level: Default::default(),
            }),
            _ => CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
        };

        Job {
            identity: JobIdentity {
                job_id: id.into(),
                correlation_id: format!("test-{id}").into(),
            },
            dispatch: JobDispatchConfig {
                command,
                lang: crate::api::LanguageSpec::Resolved(crate::api::LanguageCode3::eng()),
                num_speakers: crate::api::NumSpeakers(1),
                options,
                runtime_state: std::collections::BTreeMap::new(),
                debug_traces: false,
            },
            source: JobSourceContext {
                submitted_by: "127.0.0.1".into(),
                submitted_by_name: String::new(),
                source_dir: Default::default(),
            },
            filesystem: JobFilesystemConfig {
                filenames: filenames.into_iter().map(DisplayPath::from).collect(),
                has_chat,
                staging_dir: Default::default(),
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
                submitted_at: unix_now(),
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

    #[tokio::test]
    async fn submit_and_get() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();

        let info = store.get(&JobId::from("j1")).await;
        assert!(info.is_some());
        assert_eq!(info.unwrap().command, "morphotag");
    }

    #[tokio::test]
    async fn conflict_detection() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let job1 = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job1).await.unwrap();

        let job2 = make_job("j2", ReleasedCommand::Align, vec!["a.cha".into()]);
        let result = store.submit(job2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn cancel_job() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();

        store
            .cancel(&JobId::from("j1"), CancellationRequest::default())
            .await
            .unwrap();
        let info = store.get(&JobId::from("j1")).await.unwrap();
        assert_eq!(info.status, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn delete_completed_job() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let mut job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        job.execution.status = JobStatus::Completed;
        store.submit(job).await.unwrap();

        store.delete(&JobId::from("j1")).await.unwrap();
        assert!(store.get(&JobId::from("j1")).await.is_none());
    }

    #[tokio::test]
    async fn submit_persists_job_without_relocking_store_state() {
        let (store, db, _dir) = test_store_with_db().await;

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();

        let jobs = db.load_all_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "j1");
        assert_eq!(jobs[0].status, "queued");
    }

    #[tokio::test]
    async fn cancel_persists_status_after_releasing_store_lock() {
        let (store, db, _dir) = test_store_with_db().await;

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();
        store
            .cancel(&JobId::from("j1"), CancellationRequest::default())
            .await
            .unwrap();

        let jobs = db.load_all_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "cancelled");
        assert!(jobs[0].completed_at.is_some());
    }

    #[tokio::test]
    async fn delete_removes_db_row_and_staging_dir_after_unlocking_store() {
        let (store, db, dir) = test_store_with_db().await;
        let staging_dir = dir.path().join("staging-job");
        std::fs::create_dir_all(&staging_dir).unwrap();
        std::fs::write(staging_dir.join("artifact.txt"), "artifact").unwrap();

        let mut job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        job.execution.status = JobStatus::Completed;
        job.filesystem.staging_dir = batchalign_types::paths::ServerPath::from(staging_dir.clone());
        store.submit(job).await.unwrap();

        store.delete(&JobId::from("j1")).await.unwrap();

        let jobs = db.load_all_jobs().await.unwrap();
        assert!(jobs.is_empty());
        assert!(!staging_dir.exists());
    }

    #[tokio::test]
    async fn list_all_ordered() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let mut j1 = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        j1.schedule.submitted_at = UnixTimestamp(100.0);
        j1.execution.status = JobStatus::Completed;
        store.submit(j1).await.unwrap();

        let mut j2 = make_job("j2", ReleasedCommand::Align, vec!["b.cha".into()]);
        j2.schedule.submitted_at = UnixTimestamp(200.0);
        j2.execution.status = JobStatus::Completed;
        store.submit(j2).await.unwrap();

        let items = store.list_all().await;
        assert_eq!(items.len(), 2);
        // Newest first
        assert_eq!(items[0].job_id, "j2");
        assert_eq!(items[1].job_id, "j1");
    }

    #[tokio::test]
    async fn claim_ready_queued_jobs_orders_by_submission_time() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let mut early = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        early.schedule.submitted_at = UnixTimestamp(100.0);
        store.submit(early).await.unwrap();

        let mut late = make_job("j2", ReleasedCommand::Align, vec!["b.cha".into()]);
        late.schedule.submitted_at = UnixTimestamp(200.0);
        store.submit(late).await.unwrap();

        let poll = store.claim_ready_queued_jobs().await;
        assert_eq!(
            poll.ready_job_ids,
            vec![JobId::from("j1"), JobId::from("j2")]
        );
        assert_eq!(poll.next_wake_at, None);

        assert!(
            store
                .registry
                .runner_claim_active(&JobId::from("j1"))
                .await
                .unwrap()
        );
        assert!(
            store
                .registry
                .runner_claim_active(&JobId::from("j2"))
                .await
                .unwrap()
        );
        let lease = store
            .registry
            .lease_state(&JobId::from("j1"))
            .await
            .unwrap();
        assert_eq!(
            lease.leased_by_node.as_deref(),
            Some(store.node_id().as_ref())
        );
        assert!(lease.expires_at.is_some() && lease.heartbeat_at.is_some());
    }

    #[tokio::test]
    async fn claim_ready_queued_jobs_skips_deferred_and_reports_next_wake() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let mut ready = make_job("ready", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        ready.schedule.submitted_at = UnixTimestamp(100.0);
        store.submit(ready).await.unwrap();

        let deferred_at = UnixTimestamp(unix_now().0 + 60.0);
        let mut deferred = make_job("deferred", ReleasedCommand::Align, vec!["b.cha".into()]);
        deferred.schedule.submitted_at = UnixTimestamp(50.0);
        deferred.schedule.next_eligible_at = Some(deferred_at);
        store.submit(deferred).await.unwrap();

        let poll = store.claim_ready_queued_jobs().await;
        assert_eq!(poll.ready_job_ids, vec![JobId::from("ready")]);
        assert_eq!(poll.next_wake_at, Some(deferred_at));

        assert!(
            store
                .registry
                .runner_claim_active(&JobId::from("ready"))
                .await
                .unwrap()
        );
        assert!(
            !store
                .registry
                .runner_claim_active(&JobId::from("deferred"))
                .await
                .unwrap()
        );
        let ready_lease = store
            .registry
            .lease_state(&JobId::from("ready"))
            .await
            .unwrap();
        assert_eq!(
            ready_lease.leased_by_node.as_deref(),
            Some(store.node_id().as_ref())
        );
        assert!(
            store
                .registry
                .lease_state(&JobId::from("deferred"))
                .await
                .unwrap()
                .leased_by_node
                .is_none()
        );
    }

    #[tokio::test]
    async fn claim_ready_queued_jobs_skips_unexpired_leases_and_reclaims_expired_ones() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let now = unix_now();

        let mut leased = make_job("leased", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        leased.schedule.lease.leased_by_node = Some("other-node".into());
        leased.schedule.lease.heartbeat_at = Some(UnixTimestamp(now.0 - 10.0));
        leased.schedule.lease.expires_at = Some(UnixTimestamp(now.0 + 120.0));
        store.submit(leased).await.unwrap();

        let mut expired = make_job("expired", ReleasedCommand::Align, vec!["b.cha".into()]);
        expired.schedule.lease.leased_by_node = Some("dead-node".into());
        expired.schedule.lease.heartbeat_at = Some(UnixTimestamp(now.0 - 600.0));
        expired.schedule.lease.expires_at = Some(UnixTimestamp(now.0 - 1.0));
        store.submit(expired).await.unwrap();

        let poll = store.claim_ready_queued_jobs().await;
        assert_eq!(poll.ready_job_ids, vec![JobId::from("expired")]);
        assert_eq!(poll.next_wake_at, Some(UnixTimestamp(now.0 + 120.0)));

        let expired_lease = store
            .registry
            .lease_state(&JobId::from("expired"))
            .await
            .unwrap();
        assert_eq!(
            expired_lease.leased_by_node.as_deref(),
            Some(store.node_id().as_ref())
        );
        let leased_lease = store
            .registry
            .lease_state(&JobId::from("leased"))
            .await
            .unwrap();
        assert_eq!(leased_lease.leased_by_node.as_deref(), Some("other-node"));
    }

    #[tokio::test]
    async fn release_runner_claim_makes_job_eligible_again() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();

        let first_poll = store.claim_ready_queued_jobs().await;
        assert_eq!(first_poll.ready_job_ids, vec![JobId::from("j1")]);

        let second_poll = store.claim_ready_queued_jobs().await;
        assert!(second_poll.ready_job_ids.is_empty());

        store.release_runner_claim(&JobId::from("j1")).await;

        let after_release_poll = store.claim_ready_queued_jobs().await;
        assert_eq!(after_release_poll.ready_job_ids, vec![JobId::from("j1")]);

        store.release_runner_claim(&JobId::from("j1")).await;
        let lease = store
            .registry
            .lease_state(&JobId::from("j1"))
            .await
            .unwrap();
        assert!(lease.leased_by_node.is_none());
        assert!(lease.expires_at.is_none());
        assert!(lease.heartbeat_at.is_none());
    }

    #[tokio::test]
    async fn renew_job_lease_updates_heartbeat_and_expiry_for_local_claim() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();
        let _ = store.claim_ready_queued_jobs().await;

        let before = store
            .registry
            .lease_state(&JobId::from("j1"))
            .await
            .unwrap()
            .heartbeat_at
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(
            store.renew_job_lease(&JobId::from("j1")).await,
            crate::store::LeaseRenewalOutcome::Renewed
        );

        let lease = store
            .registry
            .lease_state(&JobId::from("j1"))
            .await
            .unwrap();
        let heartbeat_at = lease.heartbeat_at;
        let expires_at = lease.expires_at;
        assert!(heartbeat_at.unwrap() >= before);
        assert!(expires_at.unwrap() > heartbeat_at.unwrap());
    }

    #[tokio::test]
    async fn renew_job_lease_stops_after_claim_is_released() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let job = make_job("j1", ReleasedCommand::Morphotag, vec!["a.cha".into()]);
        store.submit(job).await.unwrap();
        let _ = store.claim_ready_queued_jobs().await;
        store.release_runner_claim(&JobId::from("j1")).await;

        assert_eq!(
            store.renew_job_lease(&JobId::from("j1")).await,
            crate::store::LeaseRenewalOutcome::Stop
        );
    }

    #[test]
    fn ts_iso_format() {
        let ts = UnixTimestamp(1700000000.0);
        let iso = ts_iso(ts);
        assert!(iso.starts_with("2023-11-14"));
    }

    #[test]
    fn ts_iso_invalid_timestamp_is_explicit() {
        let iso = ts_iso(UnixTimestamp(f64::INFINITY));
        assert!(iso.starts_with("invalid-unix-timestamp("));
    }

    #[test]
    fn auto_max_concurrent_caps_large_hosts() {
        assert_eq!(auto_max_concurrent_from(28, 8), 8);
    }

    #[test]
    fn auto_max_concurrent_respects_memory_tier() {
        assert_eq!(auto_max_concurrent_from(4, 1), 1);
        assert_eq!(auto_max_concurrent_from(8, 2), 2);
        assert_eq!(auto_max_concurrent_from(4, 8), 4);
    }
}
