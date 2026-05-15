//! Job lifecycle management endpoints: cancel, delete, restart.

use std::net::SocketAddr;
use std::sync::Arc;

use crate::api::{
    CallerHost, CancelSource, CancellationRecord, CancellationRequest, JobId, JobInfo, JobStatus,
};
use axum::Json;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Path, State};

use crate::AppState;
use crate::error::ServerError;

/// Request cancellation of a running or queued job.
///
/// Fires the job's `CancellationToken`, which the runner checks between files.
/// Already-terminal jobs (completed, failed, cancelled) are no-ops at the
/// state level, but the audit row is still recorded with `accepted=false`
/// so a forensic reader sees every cancel gesture (e.g., a user pressing
/// cancel twice an hour apart against a job that was already finishing).
/// Interrupted jobs can still be cancelled -- they are not considered
/// terminal here because they may be holding worker resources.
///
/// **Body** is optional. When omitted, defaults to `source=Api` with the
/// peer address as `host`. When present, fields override those defaults
/// so the TUI can attribute its cancels to `source=Tui` and identify the
/// in-flight file at the moment of the cancel.
#[utoipa::path(
    post,
    path = "/jobs/{job_id}/cancel",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    request_body(content = CancellationRequest, description = "Optional caller provenance"),
    responses(
        (status = 200, description = "Cancel request accepted", body = crate::openapi::StatusMessageResponse),
        (status = 404, description = "Job not found", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Option<Json<CancellationRequest>>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let job_id = JobId::from(job_id);

    // Enrich the body with route-known defaults: source=Api (when absent),
    // host=peer-addr (when absent or empty). A missing body parses as
    // CancellationRequest::default(), keeping backward-compatible curl
    // semantics intact.
    let mut provenance = body.map(|Json(b)| b).unwrap_or_default();
    if provenance.source.is_none() {
        provenance.source = Some(CancelSource::Api);
    }
    if provenance.host.is_none() {
        provenance.host = Some(CallerHost::from(addr.ip().to_string()));
    }

    let status = state
        .control
        .backend
        .job_status(&job_id)
        .await
        .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))?;

    // Intentionally excludes Interrupted — interrupted jobs can still be cancelled.
    if matches!(
        status,
        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
    ) {
        // Still record the cancel attempt so that "user pressed cancel
        // against an already-finished job" shows up in the audit table.
        // The accepted=false flag distinguishes it from a state-changing
        // cancel.
        let _ = state
            .control
            .backend
            .record_terminal_cancel(&job_id, provenance)
            .await;
        return Ok(Json(serde_json::json!({
            "status": status.to_string(),
            "message": "Job already finished."
        })));
    }

    state
        .control
        .backend
        .cancel_job(&job_id, provenance)
        .await?;
    Ok(Json(serde_json::json!({
        "status": "cancelled",
        "message": format!("Job {job_id} cancelled.")
    })))
}

/// Return every cancel attempt recorded against one job, oldest first.
///
/// Multi-row results indicate repeated cancel gestures (the
/// 2026-04-25 Malayalam run had two cancels exactly an hour apart).
/// `accepted=false` rows record cancels that arrived against already-
/// terminal jobs and so didn't change state — still useful forensically
/// for diagnosing "why did the operator press cancel three times" patterns.
#[utoipa::path(
    get,
    path = "/jobs/{job_id}/cancellations",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    responses(
        (status = 200, description = "Cancel audit history", body = [CancellationRecord]),
        (status = 404, description = "Job not found", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn list_job_cancellations(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<Vec<CancellationRecord>>, ServerError> {
    let job_id = JobId::from(job_id);
    // Existence check — matches cancel/delete pattern of returning 404
    // when the job ID was never seen.
    state
        .control
        .backend
        .job_status(&job_id)
        .await
        .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))?;

    let rows = state.control.backend.list_cancellations(&job_id).await?;
    let records = rows
        .into_iter()
        .map(CancellationRecord::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(records))
}

/// Permanently remove a terminal job and its associated state.
///
/// Returns 409 if the job is still running -- the caller must cancel it first.
/// Deleting a job removes it from the in-memory store and SQLite, and broadcasts
/// a `JobDeleted` event to connected WebSocket/SSE clients.
#[utoipa::path(
    delete,
    path = "/jobs/{job_id}",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    responses(
        (status = 200, description = "Job deleted", body = crate::openapi::StatusMessageResponse),
        (status = 404, description = "Job not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Job still running", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn delete_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let job_id = JobId::from(job_id);
    let is_running = state
        .control
        .backend
        .is_job_running(&job_id)
        .await
        .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))?;

    if is_running {
        return Err(ServerError::JobConflict {
            message: format!("Job {job_id} is running — cancel it first."),
            conflicts: Vec::new(),
        });
    }

    state.control.backend.delete_job(&job_id).await?;
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "message": format!("Job {job_id} deleted.")
    })))
}

/// Reset a failed or interrupted job back to `Queued` and re-run it.
///
/// Only jobs in a terminal non-cancelled state can be restarted. The store
/// resets per-file statuses and clears errors, then a fresh runner task is
/// spawned. This is the primary recovery path after transient worker crashes
/// or OOM kills.
#[utoipa::path(
    post,
    path = "/jobs/{job_id}/restart",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    responses(
        (status = 200, description = "Job restarted", body = JobInfo),
        (status = 404, description = "Job not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Job not restartable", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn restart_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Json<JobInfo>, ServerError> {
    let job_id = JobId::from(job_id);
    let info = state.control.backend.restart_job(&job_id).await?;

    Ok(Json(info))
}
