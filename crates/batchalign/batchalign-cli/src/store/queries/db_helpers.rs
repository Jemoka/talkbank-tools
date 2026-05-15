//! DB persistence helpers and WebSocket notification helpers.

use crate::api::{FileStatusEntry, FileStatusKind, JobId, JobListItem, JobStatus, UnixTimestamp};
use crate::scheduling::{AttemptOutcome, FailureCategory, RetryDisposition, WorkUnitKind};
use tracing::{debug, warn};

use super::super::JobStore;
use crate::ws::WsEvent;

/// Persisted job-level status update written through to SQLite.
pub(crate) struct PersistedJobUpdate<'a> {
    /// New durable job status.
    pub status: JobStatus,
    /// Optional job-level error message.
    pub error: Option<&'a str>,
    /// Terminal completion timestamp when present.
    pub completed_at: Option<UnixTimestamp>,
    /// Selected worker count for the run when present.
    pub num_workers: Option<i32>,
    /// Deferred retry deadline for queued jobs when present.
    pub next_eligible_at: Option<UnixTimestamp>,
}

/// Persisted file-level status update written through to SQLite.
pub(crate) struct PersistedFileUpdate<'a> {
    /// Filename within the parent job.
    pub filename: &'a str,
    /// New durable file status.
    pub status: FileStatusKind,
    /// Optional human-readable error message.
    pub error: Option<&'a str>,
    /// Optional broad error category label.
    pub error_category: Option<&'a str>,
    /// Optional linked bug-report identifier.
    pub bug_report_id: Option<&'a str>,
    /// Optional result content type for successful output.
    pub content_type: Option<&'a str>,
    /// Optional durable start timestamp.
    pub started_at: Option<UnixTimestamp>,
    /// Optional durable finish timestamp.
    pub finished_at: Option<UnixTimestamp>,
    /// Optional durable retry deadline.
    pub next_eligible_at: Option<UnixTimestamp>,
}

/// Attempt-start facts persisted for one file work unit.
pub(crate) struct AttemptStartRecord<'a> {
    /// Filename for the attempt row.
    pub filename: &'a str,
    /// Kind of work unit being attempted.
    pub work_unit_kind: WorkUnitKind,
    /// Start timestamp for the attempt row.
    pub started_at: UnixTimestamp,
}

/// Attempt-finish facts persisted for one file work unit.
pub(crate) struct AttemptFinishRecord<'a> {
    /// Filename for the attempt row.
    pub filename: &'a str,
    /// Final attempt outcome.
    pub outcome: AttemptOutcome,
    /// Optional broad failure category.
    pub failure_category: Option<FailureCategory>,
    /// Retry/terminal disposition selected by the runner.
    pub disposition: RetryDisposition,
    /// Finish timestamp for the attempt row.
    pub finished_at: UnixTimestamp,
}

impl JobStore {
    // -------------------------------------------------------------------
    // DB helper methods (safe no-ops when db is None)
    // -------------------------------------------------------------------

    /// Persist one job-level status update to SQLite when the DB is enabled.
    pub(crate) async fn db_update_job(&self, job_id: &JobId, update: PersistedJobUpdate<'_>) {
        let status_str = update.status.to_string();
        if let Some(db) = &self.db
            && let Err(e) = db
                .update_job_status(
                    job_id,
                    &status_str,
                    update.error,
                    update.completed_at.map(|ts| ts.0),
                    update.num_workers,
                    update.next_eligible_at.map(|ts| ts.0),
                )
                .await
        {
            warn!(job_id = %job_id, error = %e, "DB update_job_status failed");
        }
    }

    /// Persist one file-level status update to SQLite when the DB is enabled.
    pub(crate) async fn db_update_file(&self, job_id: &JobId, update: PersistedFileUpdate<'_>) {
        let status_str = update.status.to_string();
        if let Some(db) = &self.db
            && let Err(e) = db
                .update_file_status(
                    job_id,
                    update.filename,
                    &status_str,
                    update.error,
                    update.error_category,
                    update.bug_report_id,
                    update.content_type,
                    update.started_at.map(|ts| ts.0),
                    update.finished_at.map(|ts| ts.0),
                    update.next_eligible_at.map(|ts| ts.0),
                )
                .await
        {
            warn!(
                job_id = %job_id,
                filename = %update.filename,
                error = %e,
                "DB update_file_status failed"
            );
        }
    }

    /// Persist and attach a new active attempt record for one file.
    pub(crate) async fn db_start_attempt(&self, job_id: &JobId, attempt: AttemptStartRecord<'_>) {
        let Some(db) = &self.db else {
            return;
        };

        match db
            .insert_attempt_start(
                job_id,
                attempt.filename,
                attempt.work_unit_kind,
                attempt.started_at.0,
                None,
                None,
            )
            .await
        {
            Ok((attempt_id, _attempt_number)) => {
                let _ = self
                    .registry
                    .attach_attempt_id(job_id, attempt.filename, attempt_id)
                    .await;
            }
            Err(e) => {
                warn!(
                    job_id = %job_id,
                    filename = %attempt.filename,
                    error = %e,
                    "DB insert_attempt_start failed"
                );
            }
        }
    }

    /// Finalize the currently active attempt for one file.
    pub(crate) async fn db_finish_attempt_for_file(
        &self,
        job_id: &JobId,
        attempt: AttemptFinishRecord<'_>,
    ) {
        let attempt_id = self
            .registry
            .take_attempt_id(job_id, attempt.filename)
            .await;

        let Some(attempt_id) = attempt_id else {
            return;
        };

        if let Some(db) = &self.db
            && let Err(e) = db
                .finish_attempt(
                    &attempt_id,
                    attempt.outcome,
                    attempt.failure_category,
                    attempt.disposition,
                    attempt.finished_at.0,
                )
                .await
        {
            warn!(
                job_id = %job_id,
                filename = %attempt.filename,
                attempt_id = %attempt_id,
                error = %e,
                "DB finish_attempt failed"
            );
        }
    }

    // -------------------------------------------------------------------
    // Notifications
    // -------------------------------------------------------------------

    /// Notify WS clients of one updated job summary row.
    pub(crate) fn notify_job_item(&self, item: JobListItem) {
        match serde_json::to_value(&item) {
            Ok(job) => self.broadcast_ws_event(
                "job_update",
                WsEvent::JobUpdate { job },
                Some(item.job_id.to_string()),
                None,
            ),
            Err(error) => {
                warn!(
                    job_id = %item.job_id,
                    error = %error,
                    "Failed to serialize job update for WS broadcast"
                );
            }
        }
    }

    /// Notify WS clients of one updated file-status row.
    pub(crate) fn notify_file_update(
        &self,
        job_id: &JobId,
        file: FileStatusEntry,
        completed_files: i64,
    ) {
        match serde_json::to_value(&file) {
            Ok(file_json) => self.broadcast_ws_event(
                "file_update",
                WsEvent::FileUpdate {
                    job_id: job_id.clone(),
                    file: file_json,
                    completed_files,
                },
                Some(job_id.to_string()),
                Some(file.filename.to_string()),
            ),
            Err(error) => {
                warn!(
                    job_id = %job_id,
                    filename = %file.filename,
                    error = %error,
                    "Failed to serialize file update for WS broadcast"
                );
            }
        }
    }

    fn broadcast_ws_event(
        &self,
        event_type: &'static str,
        event: WsEvent,
        job_id: Option<String>,
        filename: Option<String>,
    ) {
        if let Err(tokio::sync::broadcast::error::SendError(_event)) = self.ws_tx.send(event) {
            debug!(
                event_type,
                job_id = job_id.as_deref().unwrap_or(""),
                filename = filename.as_deref().unwrap_or(""),
                "Dropping WS broadcast because there are no subscribers"
            );
        }
    }
}
