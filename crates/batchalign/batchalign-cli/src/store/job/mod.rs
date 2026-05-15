//! In-memory job model and lifecycle methods.
//!
//! Split into focused submodules by responsibility:
//!
//! - [`types`] — struct definitions (`Job`, `JobIdentity`, runner snapshots, etc.)
//! - [`lease`] — queue lease management (claim, renew, release, expiry)
//! - [`file_status`] — per-file status mutations (processing, done, error, retry)
//! - [`lifecycle`] — job-level state transitions (running, failed, cancelled, restart, recovery)
//! - [`projections`] — API response projections (`JobInfo`, `JobListItem`, `RunnerJobSnapshot`)
//! - [`conflict`] — file-level conflict detection between incoming and active jobs

mod conflict;
mod file_status;
mod lease;
mod lifecycle;
mod projections;
mod types;

#[cfg(test)]
pub(crate) mod test_support;

pub use conflict::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        ContentType, CorrelationId, DisplayPath, FileProgressStage, FileStatusKind, JobId,
        JobStatus, LanguageSpec, NodeId, NumSpeakers, ReleasedCommand, UnixTimestamp,
    };
    use crate::options::CommandOptions;
    use crate::store::{FileResultEntry, FileStatus};
    use std::collections::{BTreeMap, HashMap};
    use tokio_util::sync::CancellationToken;

    /// Build a small queued job for projection and conflict tests.
    fn sample_job(job_id: &str, filenames: &[&str]) -> Job {
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
                lang: LanguageSpec::Resolved(crate::api::LanguageCode3::eng()),
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

    /// Pending files exclude terminal file states.
    #[test]
    fn pending_files_skip_terminal_entries() {
        let mut job = sample_job("job-1", &["a.cha", "b.cha"]);
        job.execution.file_statuses.get_mut("a.cha").unwrap().status = FileStatusKind::Done;

        let pending = job.pending_files();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].filename.as_ref(), "b.cha");
    }

    /// Conflict detection keys on submitter and source-scoped filename.
    #[test]
    fn find_conflicts_uses_submitter_and_source_scope() {
        let mut active = sample_job("active", &["a.cha"]);
        active.execution.status = JobStatus::Running;

        let incoming = sample_job("incoming", &["a.cha"]);
        let jobs = HashMap::from([(active.identity.job_id.clone(), active)]);

        let conflicts = find_conflicts(&jobs, &incoming);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].filename, "a.cha");
        assert_eq!(conflicts[0].job_id.as_ref(), "active");
    }

    /// `find_conflicts` trusts the store HashMap it receives. Reconciliation
    /// of stale `Queued`/`Running` entries against Temporal is an upstream
    /// responsibility of `TemporalServerBackend::submit_job` (via
    /// `TemporalReconciler::reconcile_submitter`); this layer intentionally
    /// returns a conflict for any non-terminal store entry. End-to-end
    /// invariants for the reconciliation path live in
    /// `crate::temporal_reconciler::reconciler_loop_tests`.
    #[test]
    fn find_conflicts_trusts_store_state_and_does_not_reconcile() {
        let mut abandoned = sample_job("abandoned-job", &["020724a.mp3"]);
        abandoned.execution.status = JobStatus::Queued;
        abandoned.schedule.submitted_at =
            crate::api::UnixTimestamp(crate::store::unix_now().0 - 3.0 * 3600.0);
        abandoned.runtime.runner_active = false;

        let incoming = sample_job("resubmit", &["020724a.mp3"]);
        let jobs = HashMap::from([(abandoned.identity.job_id.clone(), abandoned)]);

        let conflicts = find_conflicts(&jobs, &incoming);

        assert_eq!(
            conflicts.len(),
            1,
            "find_conflicts is pure over the store HashMap. Reconciliation \
             of stale Temporal-backed jobs happens upstream in \
             `TemporalServerBackend::submit_job` via `TemporalReconciler`."
        );
    }

    /// Restart preparation keeps successful work and resets unfinished state.
    #[test]
    fn prepare_for_restart_resets_unfinished_state() {
        let mut job = sample_job("job-1", &["a.cha", "b.cha"]);
        job.execution.status = JobStatus::Failed;
        job.execution.error = Some("failed".into());
        job.execution.file_statuses.get_mut("a.cha").unwrap().status = FileStatusKind::Done;
        let retry_file = job.execution.file_statuses.get_mut("b.cha").unwrap();
        retry_file.status = FileStatusKind::Error;
        retry_file.error = Some("boom".into());
        retry_file.started_at = Some(UnixTimestamp(10.0));
        retry_file.finished_at = Some(UnixTimestamp(12.0));
        retry_file.progress_stage = Some(FileProgressStage::Aligning);
        job.execution.results.push(FileResultEntry {
            filename: DisplayPath::from("a.cha"),
            content_type: ContentType::Chat,
            error: None,
        });
        job.execution.results.push(FileResultEntry {
            filename: DisplayPath::from("b.cha"),
            content_type: ContentType::Chat,
            error: Some("boom".into()),
        });
        job.schedule.completed_at = Some(UnixTimestamp(20.0));
        job.schedule.next_eligible_at = Some(UnixTimestamp(25.0));
        job.schedule.lease.leased_by_node = Some(NodeId::from("node-1"));
        job.schedule.lease.expires_at = Some(UnixTimestamp(30.0));
        job.schedule.lease.heartbeat_at = Some(UnixTimestamp(28.0));
        job.runtime.runner_active = true;

        job.prepare_for_restart();

        assert_eq!(job.execution.status, JobStatus::Queued);
        assert_eq!(job.execution.error, None);
        assert_eq!(job.execution.completed_files, 1);
        assert_eq!(job.execution.results.len(), 1);
        assert_eq!(
            job.execution.file_statuses["a.cha"].status,
            FileStatusKind::Done
        );
        assert_eq!(
            job.execution.file_statuses["b.cha"].status,
            FileStatusKind::Queued
        );
        assert!(job.schedule.completed_at.is_none());
        assert!(job.schedule.next_eligible_at.is_none());
        assert!(job.schedule.lease.leased_by_node.is_none());
        assert!(!job.runtime.runner_active);
    }

    /// Recovery re-queues interrupted jobs when resumable file work remains.
    #[test]
    fn reconcile_recovered_runtime_state_requeues_resumable_files() {
        let mut job = sample_job("job-1", &["a.cha", "b.cha"]);
        job.execution.status = JobStatus::Running;
        job.execution.file_statuses.get_mut("a.cha").unwrap().status = FileStatusKind::Done;
        let resumable = job.execution.file_statuses.get_mut("b.cha").unwrap();
        resumable.status = FileStatusKind::Interrupted;
        resumable.started_at = Some(UnixTimestamp(10.0));
        resumable.finished_at = Some(UnixTimestamp(11.0));
        job.schedule.completed_at = Some(UnixTimestamp(20.0));
        job.schedule.next_eligible_at = Some(UnixTimestamp(21.0));
        job.schedule.lease.leased_by_node = Some(NodeId::from("node-1"));
        job.schedule.lease.expires_at = Some(UnixTimestamp(30.0));
        job.schedule.lease.heartbeat_at = Some(UnixTimestamp(28.0));

        let disposition = job.reconcile_recovered_runtime_state();

        assert_eq!(disposition, RecoveryDisposition::Requeued);
        assert_eq!(job.execution.status, JobStatus::Queued);
        assert_eq!(
            job.execution.file_statuses["b.cha"].status,
            FileStatusKind::Queued
        );
        assert!(job.execution.file_statuses["b.cha"].started_at.is_none());
        assert!(job.schedule.completed_at.is_none());
        assert!(job.schedule.lease.leased_by_node.is_none());
    }

    /// Recovery promotes all-terminal interrupted jobs to a lease-free final state.
    #[test]
    fn reconcile_recovered_runtime_state_promotes_terminal_jobs() {
        let mut job = sample_job("job-1", &["a.cha", "b.cha"]);
        job.execution.status = JobStatus::Interrupted;
        job.execution.file_statuses.get_mut("a.cha").unwrap().status = FileStatusKind::Done;
        let failed = job.execution.file_statuses.get_mut("b.cha").unwrap();
        failed.status = FileStatusKind::Error;
        failed.error = Some("boom".into());
        job.schedule.completed_at = Some(UnixTimestamp(20.0));
        job.schedule.next_eligible_at = Some(UnixTimestamp(21.0));
        job.schedule.lease.leased_by_node = Some(NodeId::from("node-1"));
        job.schedule.lease.expires_at = Some(UnixTimestamp(30.0));
        job.schedule.lease.heartbeat_at = Some(UnixTimestamp(28.0));

        let disposition = job.reconcile_recovered_runtime_state();

        assert_eq!(disposition, RecoveryDisposition::Completed);
        assert_eq!(job.execution.status, JobStatus::Completed);
        assert_eq!(job.execution.completed_files, 2);
        assert!(job.schedule.next_eligible_at.is_none());
        assert!(job.schedule.lease.leased_by_node.is_none());
        assert!(job.schedule.lease.expires_at.is_none());
    }

    /// Local queue claims and renewals stay on the job boundary.
    #[test]
    fn local_dispatch_claim_and_renew_roundtrip() {
        let mut job = sample_job("job-1", &["a.cha"]);
        let node_id = NodeId::from("node-a");
        let claimed = job
            .claim_for_local_dispatch(&node_id, UnixTimestamp(10.0), 30.0)
            .expect("claim");

        assert_eq!(claimed.leased_by_node, node_id);
        assert_eq!(claimed.heartbeat_at, UnixTimestamp(10.0));
        assert_eq!(claimed.expires_at, UnixTimestamp(40.0));
        assert!(job.runtime.runner_active);

        let renewed = job
            .renew_local_dispatch_lease(&node_id, UnixTimestamp(20.0), 30.0)
            .expect("renew");
        assert_eq!(renewed.heartbeat_at, UnixTimestamp(20.0));
        assert_eq!(renewed.expires_at, UnixTimestamp(50.0));

        job.release_local_dispatch_claim();
        assert!(!job.runtime.runner_active);
        assert!(job.schedule.lease.leased_by_node.is_none());
    }

    /// Jobs with live leases or deferrals do not report ready for dispatch.
    #[test]
    fn ready_for_local_dispatch_respects_leases_and_deferrals() {
        let mut job = sample_job("job-1", &["a.cha"]);
        let now = UnixTimestamp(10.0);
        assert!(job.ready_for_local_dispatch(now));

        job.schedule.next_eligible_at = Some(UnixTimestamp(20.0));
        assert!(!job.ready_for_local_dispatch(now));
        assert_eq!(
            job.next_local_dispatch_wake_at(now),
            Some(UnixTimestamp(20.0))
        );

        job.schedule.next_eligible_at = None;
        job.schedule.lease.leased_by_node = Some(NodeId::from("node-a"));
        job.schedule.lease.expires_at = Some(UnixTimestamp(30.0));
        assert!(!job.ready_for_local_dispatch(now));
        assert_eq!(
            job.next_local_dispatch_wake_at(now),
            Some(UnixTimestamp(30.0))
        );
    }

    /// File completion mutates file state and appends a success result.
    #[test]
    fn mark_file_done_updates_file_state() {
        let mut job = sample_job("job-1", &["a.cha"]);
        job.execution.file_statuses.get_mut("a.cha").unwrap().error = Some("stale".into());
        job.execution
            .file_statuses
            .get_mut("a.cha")
            .unwrap()
            .error_category = Some(crate::scheduling::FailureCategory::WorkerTimeout);

        assert!(job.mark_file_done(
            "a.cha",
            UnixTimestamp(12.0),
            Some(CompletedFileOutput {
                filename: DisplayPath::from("a.cha"),
                content_type: ContentType::Chat,
            })
        ));

        assert_eq!(
            job.execution.file_statuses["a.cha"].status,
            FileStatusKind::Done
        );
        assert_eq!(
            job.execution.file_statuses["a.cha"].finished_at,
            Some(UnixTimestamp(12.0))
        );
        assert!(job.execution.file_statuses["a.cha"].error.is_none());
        assert!(
            job.execution.file_statuses["a.cha"]
                .error_category
                .is_none()
        );
        assert_eq!(job.execution.completed_files, 1);
        assert_eq!(job.execution.results.len(), 1);
    }

    /// Retry scheduling stays on the job boundary.
    #[test]
    fn mark_file_retry_pending_sets_retry_metadata() {
        let mut job = sample_job("job-1", &["a.cha"]);

        assert!(job.mark_file_retry_pending(
            "a.cha",
            &FileRetryRecord {
                message: "retry".into(),
                category: crate::scheduling::FailureCategory::WorkerTimeout,
                finished_at: UnixTimestamp(11.0),
                retry_at: UnixTimestamp(20.0),
            }
        ));

        let file_status = &job.execution.file_statuses["a.cha"];
        assert_eq!(file_status.status, FileStatusKind::Processing);
        assert_eq!(file_status.next_eligible_at, Some(UnixTimestamp(20.0)));
        assert_eq!(
            file_status.progress_stage,
            Some(FileProgressStage::RetryScheduled)
        );
    }

    /// Clearing retry state also clears stale retry errors before a new attempt.
    #[test]
    fn clear_file_retry_state_clears_retry_error_metadata() {
        let mut job = sample_job("job-1", &["a.cha"]);
        assert!(job.mark_file_retry_pending(
            "a.cha",
            &FileRetryRecord {
                message: "retry".into(),
                category: crate::scheduling::FailureCategory::WorkerTimeout,
                finished_at: UnixTimestamp(11.0),
                retry_at: UnixTimestamp(20.0),
            }
        ));

        assert!(job.clear_file_retry_state("a.cha"));
        let file_status = &job.execution.file_statuses["a.cha"];
        assert!(file_status.error.is_none());
        assert!(file_status.error_category.is_none());
        assert!(file_status.finished_at.is_none());
        assert!(file_status.next_eligible_at.is_none());
    }
}
