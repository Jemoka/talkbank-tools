//! Server error types — maps to HTTP status codes.
//!
//! Error responses use `{"detail": "..."}` to match FastAPI's `HTTPException`.

#[cfg(feature = "server")]
use axum::http::StatusCode;
#[cfg(feature = "server")]
use axum::response::{IntoResponse, Response};

use crate::api::{DurationMs, JobId};

/// Detail of a single file that conflicts with an already-active job.
///
/// Returned inside the `conflicts` array of a [`ServerError::JobConflict`]
/// 409 response body. Each entry identifies exactly which file, in which
/// existing job, caused the conflict. Callers can use this to show the user
/// which files need to finish (or be cancelled) before resubmission.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConflictDetail {
    /// The filename that overlaps between the new submission and an active job.
    pub filename: crate::api::DisplayPath,
    /// The `job_id` of the existing active job that owns this file.
    pub job_id: JobId,
    /// The command the conflicting job is running.
    pub command: crate::api::ReleasedCommand,
    /// The current status of the conflicting job.
    pub status: crate::api::JobStatus,
}

/// All errors that can occur in the server.
///
/// Each variant maps to an HTTP status code via [`IntoResponse`]. The response
/// body is always `{"detail": "..."}` (matching FastAPI's `HTTPException`
/// convention), except for [`JobConflict`](Self::JobConflict) which includes
/// a structured `conflicts` array.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// A database operation failed (schema migration, insert, query, etc.).
    ///
    /// **HTTP 500.** Callers should retry or report the error. Typically
    /// indicates a corrupt DB, disk-full condition, or schema mismatch.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// A database migration failed.
    ///
    /// **HTTP 500.** Indicates a schema version mismatch or corrupt migration state.
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// Persisted structured data could not be serialized or deserialized.
    ///
    /// **HTTP 500.** Indicates an internal schema/shape mismatch or corrupt
    /// stored JSON payload in SQLite.
    #[error("persistence error: {0}")]
    Persistence(String),

    /// The requested `job_id` does not exist in the [`JobStore`](crate::store::JobStore).
    ///
    /// **HTTP 404.** Callers should verify the job ID. The job may have been
    /// pruned after expiry (`job_ttl_days`) or explicitly deleted.
    #[error("job {0} not found")]
    JobNotFound(JobId),

    /// A new job submission overlaps with files already being processed by
    /// an active job from the same submitter.
    ///
    /// **HTTP 409.** The `conflicts` field lists each overlapping file and
    /// the active job that owns it. Callers should wait for the conflicting
    /// job to finish, cancel it, or remove the overlapping files from the
    /// new submission.
    #[error("{message}")]
    JobConflict {
        /// Human-readable description of the conflict.
        message: String,
        /// Per-file details showing which active jobs overlap.
        conflicts: Vec<ConflictDetail>,
    },

    /// An operation (e.g. restart, delete) was attempted on a job that is
    /// still queued or running and has not yet reached a terminal state.
    ///
    /// **HTTP 409.** Callers should cancel the job first, or wait for it
    /// to complete before retrying the operation.
    #[error("job {0} is not in a terminal state")]
    JobNotTerminal(JobId),

    /// A result file was requested (e.g. `GET /jobs/{id}/results/{filename}`)
    /// but the file does not exist on disk.
    ///
    /// **HTTP 404.** The job may not have produced output for this file,
    /// or the staging directory may have been cleaned up.
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// A result file was requested but the file has not finished processing
    /// yet (still queued or in progress).
    ///
    /// **HTTP 409.** Callers should poll the job status and retry once the
    /// file reaches `"done"` status.
    #[error("file not ready: {0}")]
    FileNotReady(String),

    /// The submitted command name is not recognized by the server (not in
    /// the set of worker-advertised capabilities).
    ///
    /// **HTTP 400.** Callers should check `GET /health` for the list of
    /// supported `capabilities` and resubmit with a valid command.
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    /// A request failed input validation (e.g. empty filename list, missing
    /// required fields, invalid language code).
    ///
    /// **HTTP 400.** Callers should fix the request payload and resubmit.
    #[error("validation error: {0}")]
    Validation(String),

    /// A Python worker process failed (crashed, timed out, or returned an
    /// error response over the stdio IPC protocol).
    ///
    /// **HTTP 500.** The worker pool will automatically restart crashed
    /// workers. Callers can retry the job.
    #[error("worker error: {0}")]
    Worker(#[from] crate::worker::error::WorkerError),

    /// A filesystem I/O operation failed (reading/writing staging files,
    /// creating directories, etc.).
    ///
    /// **HTTP 500.** Typically indicates a permissions problem or full disk.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The system's available memory is below the critical threshold
    /// (configurable via `ServerConfig.memory_gate_mb`, default 2048 MB)
    /// and no idle workers can be reused for the requested command.
    ///
    /// **HTTP 500.** The memory gate polls every 5 seconds and waits up to
    /// 120 seconds for memory to free up before triggering this error.
    /// Callers should wait and retry, or reduce the number of concurrent
    /// jobs.
    #[error("memory pressure: {0}")]
    MemoryPressure(String),

    /// An FA audio segment request produced zero samples because the requested
    /// time window falls past the end of the source audio file.
    ///
    /// This is not a fatal error at the file level: the FA pipeline handles it
    /// by leaving the affected group's words unaligned rather than aborting.
    /// It is surfaced as a `ServerError` variant so the transport layer can
    /// match on it without inspecting error message strings.
    #[error("empty FA audio segment [{start_ms}ms..{end_ms}ms) in {path}")]
    EmptyFaAudioSegment {
        /// Source media path.
        path: String,
        /// Requested segment start.
        start_ms: DurationMs,
        /// Requested segment end.
        end_ms: DurationMs,
    },

    /// The runner was asked to execute a job that is not present in this
    /// server's local `JobStore`.
    ///
    /// **HTTP 500 — internal consistency error, not a 404.** This is
    /// architecturally impossible under the per-host Temporal task-queue
    /// topology (each server's task queue is unique, so only the submitting
    /// server's worker ever polls its own activities). Surfacing this error
    /// indicates either (a) a misconfigured shared task queue — a regression
    /// of the 2026-04-15 fleet-worker bug — or (b) the store was concurrently
    /// truncated while a workflow was in-flight. Either case must fail
    /// loudly; silently reporting success would mask a real correctness bug.
    #[error(
        "job {0} is not in this server's local JobStore — activity may have \
         been routed to the wrong server (check task-queue configuration)"
    )]
    JobNotInLocalStore(JobId),
}

#[cfg(feature = "server")]
impl ServerError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Database(_) | Self::Migration(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Persistence(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::JobNotFound(_) => StatusCode::NOT_FOUND,
            Self::JobConflict { .. } => StatusCode::CONFLICT,
            Self::JobNotTerminal(_) => StatusCode::CONFLICT,
            Self::FileNotFound(_) => StatusCode::NOT_FOUND,
            Self::FileNotReady(_) => StatusCode::CONFLICT,
            Self::UnknownCommand(_) => StatusCode::BAD_REQUEST,
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Worker(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::MemoryPressure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // EmptyFaAudioSegment is an internal skip signal, never returned as HTTP.
            Self::EmptyFaAudioSegment { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            // JobNotInLocalStore is an internal consistency error, not a
            // user-facing 404. It only surfaces during Temporal activity
            // dispatch and propagates through the activity handler as a
            // non-retryable error, so HTTP mapping is defensive but rare.
            Self::JobNotInLocalStore(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[cfg(feature = "server")]
impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = match &self {
            Self::JobConflict { message, conflicts } => {
                serde_json::json!({
                    "detail": {
                        "message": message,
                        "conflicts": conflicts,
                    }
                })
            }
            _ => serde_json::json!({ "detail": self.to_string() }),
        };
        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_not_found_is_404() {
        let err = ServerError::JobNotFound(JobId::from("abc123"));
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.to_string(), "job abc123 not found");
    }

    #[test]
    fn validation_is_400() {
        let err = ServerError::Validation("bad input".into());
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn conflict_is_409() {
        let err = ServerError::JobConflict {
            message: "files overlap".into(),
            conflicts: vec![ConflictDetail {
                filename: crate::api::DisplayPath::from("a.cha"),
                job_id: JobId::from("j1"),
                command: crate::api::ReleasedCommand::Morphotag,
                status: crate::api::JobStatus::Running,
            }],
        };
        assert_eq!(err.status_code(), StatusCode::CONFLICT);
    }

    #[test]
    fn error_response_has_detail_field() {
        let err = ServerError::JobNotFound(JobId::from("abc"));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// `JobNotInLocalStore` is an internal consistency error (Temporal
    /// activity landed on the wrong server), NOT a user-facing missing
    /// resource. It must map to 500, not 404 — a 404 would signal to a
    /// caller that the job was pruned or never existed, when in fact it
    /// exists on a different server. The message must also explicitly
    /// point operators at task-queue configuration.
    #[test]
    fn job_not_in_local_store_is_500_and_mentions_task_queue() {
        let err = ServerError::JobNotInLocalStore(JobId::from("abc123"));
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        let rendered = err.to_string();
        assert!(
            rendered.contains("abc123"),
            "error message should include the job id: {rendered}"
        );
        assert!(
            rendered.contains("task-queue"),
            "error message should direct operators at task-queue config: {rendered}"
        );
    }
}
