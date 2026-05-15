//! Job lifecycle mutations: submit, restart, cancel, interrupt_all_for_shutdown.

use crate::api::{CancellationRequest, JobId, JobInfo, JobStatus};
use crate::db::NewJobRecord;
use tracing::{info, warn};

use super::super::job::Job;
use super::super::{JobStore, PersistedJobUpdate, unix_now};
use crate::error::ServerError;

impl JobStore {
    /// Register a job. Returns an error if conflicts are detected.
    pub async fn submit(&self, job: Job) -> Result<(), ServerError> {
        let persist = NewJobRecord {
            job_id: String::from(job.identity.job_id.clone()),
            correlation_id: job.identity.correlation_id.to_string(),
            command: job.dispatch.command.to_string(),
            lang: job.dispatch.lang.to_string(),
            num_speakers: job.dispatch.num_speakers.0,
            status: job.execution.status.to_string(),
            staging_dir: job.filesystem.staging_dir.to_string(),
            filenames: job
                .filesystem
                .filenames
                .iter()
                .cloned()
                .map(String::from)
                .collect(),
            has_chat: job.filesystem.has_chat.clone(),
            options: job.dispatch.options.clone(),
            media_mapping: job.filesystem.media_mapping.to_string(),
            media_subdir: job.filesystem.media_subdir.to_string(),
            source_dir: job.source.source_dir.as_str().to_owned(),
            submitted_by: job.source.submitted_by.clone(),
            submitted_by_name: job.source.submitted_by_name.clone(),
            submitted_at: job.schedule.submitted_at.0,
            paths_mode: job.filesystem.paths_mode,
            source_paths: job
                .filesystem
                .source_paths
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            output_paths: job
                .filesystem
                .output_paths
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
        };
        let job_id = job.identity.job_id.clone();
        let correlation_id = job.identity.correlation_id.clone();
        let command = job.dispatch.command;
        let total_files = job.total_files();
        self.registry.insert_checked(job).await?;

        // Persist to DB
        if let Some(db) = &self.db
            && let Err(e) = db.insert_job(&persist).await
        {
            warn!(job_id = %job_id, error = %e, "Failed to persist job to DB");
        }

        info!(
            job_id = %job_id,
            correlation_id = %correlation_id,
            command = %command,
            total_files = total_files,
            "Job queued"
        );

        Ok(())
    }

    /// Restart a cancelled or failed job — reset file statuses and re-queue.
    pub async fn restart(&self, job_id: &JobId) -> Result<JobInfo, ServerError> {
        let info = self
            .registry
            .restart_job(job_id)
            .await
            .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))??;
        self.notify_job_item(info.job_update);

        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: JobStatus::Queued,
                error: None,
                completed_at: None,
                num_workers: None,
                next_eligible_at: None,
            },
        )
        .await;
        if let Some(db) = &self.db
            && let Err(e) = db.update_job_lease(job_id, None, None, None).await
        {
            warn!(job_id = %job_id, error = %e, "DB update_job_lease failed on restart");
        }

        Ok(info.info)
    }

    /// Interrupt all active (running or queued) jobs as part of graceful server shutdown.
    ///
    /// Returns the number of jobs interrupted.  Jobs land in
    /// `JobStatus::Interrupted` (resumable) rather than `JobStatus::Cancelled`
    /// (terminal), so the startup recovery path requeues unfinished work.
    pub async fn interrupt_all_for_shutdown(&self) -> usize {
        self.registry.interrupt_all_active(unix_now()).await
    }

    /// Records a cancellation audit row with `accepted = true`, without
    /// changing the job's lifecycle status.
    ///
    /// Counterpart to [`JobStore::record_terminal_cancel`], which writes
    /// `accepted = false` for cancel attempts against an already-terminal
    /// job. Both wrap the same private `record_audit_row` primitive; the
    /// `accepted` column is what audit readers use to distinguish "server
    /// saw this job as active and genuinely interrupted it" from "user
    /// pressed Cancel against an already-finished job."
    ///
    /// Used by the shutdown path (Temporal backend) to leave a forensic
    /// record in the `cancellations` table BEFORE flipping the lifecycle
    /// bit to `Interrupted` via `interrupt_all_for_shutdown`. Keeping
    /// audit and lifecycle as two separate operations is what lets the
    /// startup recovery flow distinguish system-initiated shutdown
    /// cancels from user gestures (see
    /// `temporal_reconciler::is_system_initiated_shutdown_cancel`).
    pub async fn record_cancellation_audit(
        &self,
        job_id: &JobId,
        provenance: &CancellationRequest,
    ) {
        self.record_audit_row(job_id, provenance, unix_now(), /* accepted = */ true)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::broadcast;

    use super::*;
    use crate::api::{
        CancelReason, CancelSource, ContentType, FileStatusKind, JobId, ReleasedCommand,
    };
    use crate::db::JobDB;
    use crate::store::queries::tests::{make_job, test_config};
    use crate::store::{FileResultEntry, JobStore, UnixTimestamp};
    use crate::ws::BROADCAST_CAPACITY;

    /// `interrupt_all_for_shutdown` must transition every active job to
    /// `JobStatus::Interrupted` and return the count of transitioned jobs.
    ///
    /// This test proves that the public entry point delegates to
    /// `registry.interrupt_all_active` with the correct semantics rather than
    /// (incorrectly) landing jobs in `JobStatus::Cancelled`.
    #[tokio::test]
    async fn interrupt_all_for_shutdown_marks_active_jobs_interrupted() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        // Submit two jobs and advance both to Running so they are "active".
        let job_a = make_job(
            "shutdown-a",
            ReleasedCommand::Morphotag,
            vec!["a.cha".into()],
        );
        let job_b = make_job(
            "shutdown-b",
            ReleasedCommand::Morphotag,
            vec!["b.cha".into()],
        );
        store.submit(job_a).await.expect("submit job-a");
        store.submit(job_b).await.expect("submit job-b");

        store.mark_job_running(&JobId::from("shutdown-a")).await;
        store.mark_job_running(&JobId::from("shutdown-b")).await;

        // Drive the shutdown path.
        let interrupted = store.interrupt_all_for_shutdown().await;
        assert_eq!(interrupted, 2, "both running jobs should be interrupted");

        // Both jobs must land in Interrupted (resumable), not Cancelled (terminal).
        let info_a = store
            .get(&JobId::from("shutdown-a"))
            .await
            .expect("job-a after shutdown");
        let info_b = store
            .get(&JobId::from("shutdown-b"))
            .await
            .expect("job-b after shutdown");

        assert_eq!(
            info_a.status,
            JobStatus::Interrupted,
            "job-a must be Interrupted, not Cancelled"
        );
        assert_eq!(
            info_b.status,
            JobStatus::Interrupted,
            "job-b must be Interrupted, not Cancelled"
        );

        // completed_at must be set (Task 1 established this invariant in
        // interrupt_for_shutdown / interrupt_all_active).
        assert!(
            info_a.completed_at.is_some(),
            "job-a should have completed_at set after interrupt"
        );
        assert!(
            info_b.completed_at.is_some(),
            "job-b should have completed_at set after interrupt"
        );
    }

    #[tokio::test]
    async fn restart_preserves_successful_morphotag_results_and_requeues_failed_files() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let mut job = make_job(
            "morphotag-restart",
            ReleasedCommand::Morphotag,
            vec!["eng.cha".into(), "missing_lang.cha".into()],
        );
        job.execution.status = JobStatus::Failed;
        job.execution.error = Some("worker dispatch failed".into());
        let eng = job
            .execution
            .file_statuses
            .get_mut("eng.cha")
            .expect("english status");
        eng.status = FileStatusKind::Done;
        eng.finished_at = Some(UnixTimestamp(10.0));

        let missing = job
            .execution
            .file_statuses
            .get_mut("missing_lang.cha")
            .expect("missing lang status");
        missing.status = FileStatusKind::Error;
        missing.error = Some("worker dispatch failed".into());
        missing.finished_at = Some(UnixTimestamp(12.0));

        job.execution.results.push(FileResultEntry {
            filename: "eng.cha".into(),
            content_type: ContentType::Chat,
            error: None,
        });
        job.execution.results.push(FileResultEntry {
            filename: "missing_lang.cha".into(),
            content_type: ContentType::Chat,
            error: Some("worker dispatch failed".into()),
        });
        job.execution.completed_files = 2;

        store.submit(job).await.expect("submit test job");

        let restarted = store
            .restart(&JobId::from("morphotag-restart"))
            .await
            .expect("restart failed morphotag job");
        assert_eq!(restarted.status, JobStatus::Queued);

        let restarted_eng = restarted
            .file_statuses
            .iter()
            .find(|file| file.filename.as_ref() == "eng.cha")
            .expect("restarted english file status");
        let restarted_missing = restarted
            .file_statuses
            .iter()
            .find(|file| file.filename.as_ref() == "missing_lang.cha")
            .expect("restarted failed file status");

        assert_eq!(restarted_eng.status, FileStatusKind::Done);
        assert_eq!(restarted_missing.status, FileStatusKind::Queued);
        assert!(restarted_eng.error.is_none());
        assert!(restarted_missing.error.is_none());

        let detail = store
            .get_job_detail(&JobId::from("morphotag-restart"))
            .await
            .expect("job detail after restart");
        assert_eq!(
            detail.results.len(),
            1,
            "restart should retain only successful results"
        );
        assert_eq!(detail.results[0].filename.as_ref(), "eng.cha");
        assert!(detail.results[0].error.is_none());

        let snapshot = store
            .runner_snapshot(&JobId::from("morphotag-restart"))
            .await
            .expect("runner snapshot after restart");
        assert_eq!(
            snapshot.pending_files.len(),
            1,
            "restart should queue only the failed morphotag file for rerun"
        );
        assert_eq!(
            snapshot.pending_files[0].filename.as_ref(),
            "missing_lang.cha"
        );
    }

    /// The Temporal-backend shutdown path must:
    ///   (a) write a `cancellations` audit row with source=Signal,
    ///       reason="server-cancel-all" for every active job, AND
    ///   (b) leave every active job in `JobStatus::Interrupted`
    ///       (recovery-eligible), NOT `JobStatus::Cancelled` (terminal).
    ///
    /// This test exercises the store-level primitives (`record_cancellation_audit`
    /// + `interrupt_all_for_shutdown`) that the Temporal backend's rewritten body
    /// calls.  Constructing a live `TemporalServerBackend` would require a real
    /// Temporal server; testing via the store proves both invariants without that
    /// dependency.
    #[tokio::test]
    async fn interrupt_all_for_shutdown_writes_interrupted_status_with_signal_audit() {
        // Use a DB-backed store so `list_cancellations` can read the persisted rows.
        let dir = tempfile::tempdir().expect("create temp dir for shutdown audit test");
        let db = Arc::new(
            JobDB::open(Some(dir.path()))
                .await
                .expect("open JobDB for shutdown audit test"),
        );
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), Some(db), tx);

        // Submit two jobs and advance both to Running so they are "active".
        let job_a = make_job(
            "shutdown-sig-a",
            ReleasedCommand::Morphotag,
            vec!["a.cha".into()],
        );
        let job_b = make_job(
            "shutdown-sig-b",
            ReleasedCommand::Morphotag,
            vec!["b.cha".into()],
        );
        store.submit(job_a).await.expect("submit job-a");
        store.submit(job_b).await.expect("submit job-b");
        store.mark_job_running(&JobId::from("shutdown-sig-a")).await;
        store.mark_job_running(&JobId::from("shutdown-sig-b")).await;

        // Snapshot the active job IDs (mirrors what the Temporal backend does).
        let active_ids: Vec<JobId> = store
            .list_all()
            .await
            .into_iter()
            .filter(|j| j.status.can_cancel())
            .map(|j| j.job_id)
            .collect();
        assert_eq!(
            active_ids.len(),
            2,
            "both jobs should be active before shutdown"
        );

        // Step 1: write audit rows (the new Temporal-backend body does this before
        // flipping lifecycle, so the DB row exists regardless of ordering race).
        let provenance = CancellationRequest {
            source: Some(CancelSource::Signal),
            reason: Some(CancelReason::server_cancel_all()),
            ..Default::default()
        };
        for job_id in &active_ids {
            store.record_cancellation_audit(job_id, &provenance).await;
        }

        // Step 2: flip lifecycle to Interrupted.
        let count = store.interrupt_all_for_shutdown().await;
        assert_eq!(count, 2, "both jobs should be interrupted");

        // Assert (b): both jobs are now Interrupted, not Cancelled.
        let info_a = store
            .get(&JobId::from("shutdown-sig-a"))
            .await
            .expect("job-a after shutdown");
        let info_b = store
            .get(&JobId::from("shutdown-sig-b"))
            .await
            .expect("job-b after shutdown");
        assert_eq!(
            info_a.status,
            JobStatus::Interrupted,
            "shutdown handler must mark Interrupted, not Cancelled"
        );
        assert_eq!(
            info_b.status,
            JobStatus::Interrupted,
            "shutdown handler must mark Interrupted, not Cancelled"
        );

        // Assert (a): audit rows exist with source=Signal and reason="server-cancel-all".
        let rows_a = store
            .list_cancellations(&JobId::from("shutdown-sig-a"))
            .await
            .expect("list_cancellations for job-a");
        let rows_b = store
            .list_cancellations(&JobId::from("shutdown-sig-b"))
            .await
            .expect("list_cancellations for job-b");

        assert!(!rows_a.is_empty(), "audit row(s) written for job-a");
        assert!(!rows_b.is_empty(), "audit row(s) written for job-b");

        let all_rows: Vec<_> = rows_a.iter().chain(rows_b.iter()).collect();
        let server_cancel_all_count = all_rows
            .iter()
            .filter(|r| {
                r.source == "signal"
                    && r.reason.as_deref() == Some("server-cancel-all")
                    && r.accepted
            })
            .count();
        assert_eq!(
            server_cancel_all_count, 2,
            "exactly one server-cancel-all audit row per active job"
        );
    }

    /// End-to-end pin: shutdown of an in-flight job leaves the local DB row
    /// in a state that the next server-startup recovery sequence transitions
    /// to `Queued` (resumable), not `Cancelled` (terminal).
    ///
    /// Sequence under test:
    ///   1. Server is up; a job is submitted and marked Running.
    ///   2. Server shuts down — `interrupt_all_for_shutdown` writes the row
    ///      as `Interrupted`.
    ///   3. Process exits (we drop the store).
    ///   4. Server restarts — `db.recover_interrupted()` runs (a no-op for
    ///      already-Interrupted rows; included to faithfully match the
    ///      production startup flow at `server.rs:112`).
    ///   5. `store.load_from_db()` reads the row, sees `Interrupted` plus a
    ///      resumable file, and transitions the job to `Queued`, writing
    ///      the new status back to disk.
    ///
    /// The 2026-04-27 bug was that step 2 wrote `Cancelled` instead of
    /// `Interrupted`. Step 5's `load_from_db` only reconciles
    /// `Interrupted | Running` rows (per `store/queries/recovery.rs:307`),
    /// so a `Cancelled` row was never revisited and the job stayed
    /// permanently dead. This test pins the corrected end-to-end shape.
    #[tokio::test]
    async fn shutdown_interrupted_job_recovers_to_queued_after_restart() {
        // Use a persistent tempdir so the DB survives across the simulated
        // server restart. We open two distinct `JobStore` instances against
        // the same on-disk DB to mimic process exit and re-launch.
        let dir = tempfile::tempdir().expect("create temp dir for restart-recovery test");

        // ---- First server lifecycle: submit, run, shutdown ----
        {
            let db = Arc::new(
                JobDB::open(Some(dir.path()))
                    .await
                    .expect("open JobDB for first lifecycle"),
            );
            let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
            let store = JobStore::new(test_config(), Some(db), tx);

            let job = make_job(
                "shutdown-recover",
                ReleasedCommand::Morphotag,
                vec!["a.cha".into()],
            );
            store.submit(job).await.expect("submit shutdown-recover");
            store
                .mark_job_running(&JobId::from("shutdown-recover"))
                .await;

            let interrupted = store.interrupt_all_for_shutdown().await;
            assert_eq!(
                interrupted, 1,
                "the running job should be interrupted at shutdown"
            );

            let info = store
                .get(&JobId::from("shutdown-recover"))
                .await
                .expect("job after shutdown");
            assert_eq!(
                info.status,
                JobStatus::Interrupted,
                "shutdown must leave the row Interrupted, not Cancelled"
            );
            // store goes out of scope here, simulating server process exit.
        }

        // ---- Second server lifecycle: startup recovery ----
        let db = Arc::new(
            JobDB::open(Some(dir.path()))
                .await
                .expect("re-open JobDB after restart"),
        );
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), Some(db.clone()), tx);

        // Faithfully mirror server.rs:112 — recover_interrupted runs first.
        // It is a no-op for already-Interrupted rows (it only matches
        // 'queued' and 'running'), but a real startup always calls it.
        db.recover_interrupted()
            .await
            .expect("recover_interrupted at restart");

        // load_from_db is the second step. It reads the Interrupted row,
        // calls reconcile_recovered_runtime_state, sees the resumable file,
        // and writes status=Queued back to disk.
        store.load_from_db().await.expect("load_from_db at restart");

        let recovered = store
            .get(&JobId::from("shutdown-recover"))
            .await
            .expect("job after recovery");
        assert_eq!(
            recovered.status,
            JobStatus::Queued,
            "recovery must transition Interrupted (resumable) to Queued"
        );

        // The recovered file should also be back to Queued, not stuck in any
        // intermediate state. This mirrors what
        // `reconcile_recovered_runtime_state` does for resumable files.
        let recovered_file = recovered
            .file_statuses
            .iter()
            .find(|f| f.filename.as_ref() == "a.cha")
            .expect("recovered file status");
        assert_eq!(recovered_file.status, FileStatusKind::Queued);
        assert!(recovered_file.started_at.is_none());
    }
}
