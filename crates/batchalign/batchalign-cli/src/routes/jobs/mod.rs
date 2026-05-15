//! Job lifecycle endpoints.
//!
//! Covers the full lifecycle of a processing job: submission, polling, result
//! retrieval, cancellation, deletion, restart, and real-time SSE streaming.
//! All handlers share an `Arc<AppState>` and coordinate through the in-memory
//! `JobStore` (backed by SQLite for crash recovery).

pub(crate) mod detail;
pub(crate) mod lifecycle;
pub(crate) mod stream;

pub(crate) use detail::{get_job, get_results, get_single_result};
pub(crate) use lifecycle::{cancel_job, delete_job, list_job_cancellations, restart_job};
pub(crate) use stream::stream_job;

// Re-export utoipa-generated path structs so that the `OpenApi` derive in
// `openapi.rs` can resolve them at `crate::routes::jobs::__path_*`.
#[allow(unused_imports)]
pub(crate) use detail::{__path_get_job, __path_get_results, __path_get_single_result};
#[allow(unused_imports)]
pub(crate) use lifecycle::{
    __path_cancel_job, __path_delete_job, __path_list_job_cancellations, __path_restart_job,
};
#[allow(unused_imports)]
pub(crate) use stream::__path_stream_job;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::api::{CorrelationId, JobInfo, JobSubmission, ReleasedCommand};
use axum::extract::State;
use axum::extract::connect_info::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use tracing::info;

use crate::AppState;
use crate::error::ServerError;
use crate::hostname::resolve_hostname;
use crate::submission::{SubmissionContext, materialize_submission_job};

/// Build the jobs router with all job lifecycle endpoints.
///
/// Registers routes for submission, listing, detail, results retrieval,
/// per-file results, cancellation, deletion, restart, and SSE streaming.
///
/// The POST `/jobs` route disables axum's built-in 2 MB `Json` extractor
/// limit so that large batch submissions (hundreds of CHAT files) are
/// governed solely by the outer `RequestBodyLimitLayer` configured via
/// `max_body_bytes_mb` in `server.yaml` (default 100 MB).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/jobs", post(submit_job))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{job_id}", get(get_job))
        .route("/jobs/{job_id}/results", get(get_results))
        .route("/jobs/{job_id}/results/{*filename}", get(get_single_result))
        .route("/jobs/{job_id}/cancel", post(cancel_job))
        .route("/jobs/{job_id}/cancellations", get(list_job_cancellations))
        .route("/jobs/{job_id}", delete(delete_job))
        .route("/jobs/{job_id}/restart", post(restart_job))
        .route("/jobs/{job_id}/stream", get(stream_job))
        // Disable axum's built-in 2 MB `Json` extractor limit on all job
        // routes.  The outer `RequestBodyLimitLayer` (configured via
        // `max_body_bytes_mb`, default 100 MB) remains the sole body-size
        // guard.  Without this, large batch submissions (hundreds of CHAT
        // files in one POST /jobs) hit the 2 MB default before reaching
        // our configurable limit.
        .layer(axum::extract::DefaultBodyLimit::disable())
}

/// Maximum length for a sanitized correlation ID.
///
/// Client-supplied `X-Request-Id` values are truncated to this length after
/// stripping non-safe characters, so that log lines and database rows stay
/// bounded even if a client sends a very long header.
const CORRELATION_ID_MAX_LEN: usize = 128;

fn command_supported(command: ReleasedCommand, capabilities: &[String]) -> bool {
    capabilities.iter().any(|c| c.as_str() == command.as_ref())
}

fn sanitize_correlation_id(raw: &str) -> Option<CorrelationId> {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':') {
            out.push(ch);
            if out.len() >= CORRELATION_ID_MAX_LEN {
                break;
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(CorrelationId::from(out))
    }
}

fn correlation_id_from_headers(headers: &HeaderMap, fallback: &str) -> CorrelationId {
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .and_then(sanitize_correlation_id)
        .unwrap_or_else(|| CorrelationId::from(fallback.to_string()))
}

fn supported_command_list(capabilities: &[String]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for c in capabilities {
        if c != "test-echo" {
            set.insert(c.clone());
        }
    }
    set.into_iter().collect()
}

/// Accept a new processing job and begin execution.
///
/// Validates the command against built-in tasks and worker-advertised capabilities,
/// stages input files (content mode) or records source paths (paths mode), detects
/// `(submitted_by, filename)` conflicts with active jobs, and spawns a background
/// runner task that acquires workers and dispatches files. The response echoes the
/// job back as `JobInfo` and includes the correlation ID in `X-Request-Id`.
#[utoipa::path(
    post,
    path = "/jobs",
    tag = "jobs",
    request_body = JobSubmission,
    responses(
        (status = 200, description = "Submitted job", body = JobInfo),
        (status = 400, description = "Validation error", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn submit_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(submission): Json<JobSubmission>,
) -> Result<impl IntoResponse, ServerError> {
    // Validate command
    if !command_supported(submission.command, &state.workers.capabilities) {
        let supported = supported_command_list(&state.workers.capabilities);
        return Err(ServerError::UnknownCommand(format!(
            "Unknown command: {}. Valid commands: {:?}",
            submission.command, supported
        )));
    }

    // Reject malformed (command, lang) pairings BEFORE materializing the
    // job. Morphotag, translate, and coref must arrive with
    // `LanguageSpec::PerFile`; every other processing command must arrive
    // with `Auto` or `Resolved(_)`. This is the single chokepoint that
    // keeps job records honest and stops the 2026-05-03 lang-placeholder
    // leak from ever recurring at the wire boundary.
    submission
        .validate()
        .map_err(|e| ServerError::Validation(e.to_string()))?;

    // Authoritative language validation using the Stanza capability
    // registry (when populated from a worker's resources.json report).
    // This supersedes the hardcoded fallback table in submission.validate().
    if let Some(registry) = state.workers.pool.stanza_registry() {
        crate::types::request::validate_language_with_registry(&submission, Some(registry))
            .map_err(|e| ServerError::Validation(e.to_string()))?;
    }

    let job_id = uuid::Uuid::new_v4().to_string()[..12].to_string();
    let correlation_id = correlation_id_from_headers(&headers, &job_id);
    let job = materialize_submission_job(
        &submission,
        &SubmissionContext {
            job_id: job_id.clone().into(),
            correlation_id: correlation_id.clone(),
            jobs_dir: state.environment.paths.jobs_dir.clone().into(),
            submitted_by: addr.ip().to_string(),
            submitted_by_name: resolve_hostname(&addr.ip()),
        },
    )
    .await?;

    info!(
        job_id = %job_id,
        correlation_id = %correlation_id,
        command = %submission.command,
        total_files = job.total_files(),
        paths_mode = job.filesystem.paths_mode,
        submitted_by = %addr.ip(),
        "Job submission accepted"
    );

    let info = job.to_info();
    state.control.backend.submit_job(job).await?;

    let mut response_headers = HeaderMap::new();
    if let Ok(request_id) = HeaderValue::from_str(&correlation_id) {
        response_headers.insert("x-request-id", request_id);
    }

    Ok((StatusCode::OK, response_headers, Json(info)))
}

/// Return a summary of every job the server knows about (active, completed,
/// failed, or cancelled).
///
/// Used by the dashboard and CLI `jobs` command. The response is intentionally
/// compact -- per-file detail is omitted and available via `GET /jobs/{id}`.
#[utoipa::path(
    get,
    path = "/jobs",
    tag = "jobs",
    responses(
        (status = 200, description = "List all jobs", body = [crate::api::JobListItem])
    )
)]
pub(crate) async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::api::JobListItem>> {
    Json(state.control.backend.list_jobs().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertised_command_is_supported() {
        assert!(command_supported(
            ReleasedCommand::Morphotag,
            &["morphotag".to_string()]
        ));
    }

    #[test]
    fn command_not_in_capabilities_is_rejected() {
        assert!(!command_supported(
            ReleasedCommand::Morphotag,
            &["align".to_string()]
        ));
    }

    #[test]
    fn correlation_id_uses_x_request_id_when_valid() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-request-id",
            axum::http::HeaderValue::from_static("req-123_abc"),
        );
        let cid = correlation_id_from_headers(&headers, "fallback");
        assert_eq!(cid, "req-123_abc");
    }

    #[test]
    fn correlation_id_falls_back_when_missing() {
        let headers = HeaderMap::new();
        let cid = correlation_id_from_headers(&headers, "job123");
        assert_eq!(cid, "job123");
    }

    #[test]
    fn correlation_id_sanitizes_invalid_chars() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-request-id",
            axum::http::HeaderValue::from_static("abc$%/def"),
        );
        let cid = correlation_id_from_headers(&headers, "job123");
        assert_eq!(cid, "abcdef");
    }
}
