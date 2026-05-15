//! OpenAPI schema generation for the Rust control-plane server.
//!
//! Provides [`utoipa`]-based OpenAPI 3.0 schema definitions for all REST
//! endpoints. The schema is built from route `#[utoipa::path]` annotations
//! across the `routes` module and shared response types defined here.
//!
//! The generated JSON is canonical (keys sorted deterministically) so that
//! it can be checked into version control and diffed meaningfully. The
//! `batchalign-bin openapi` command writes the schema to a file, and
//! [`check_openapi_json`] verifies it stays in sync.

use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};
use utoipa::OpenApi;

/// Standard error response body, matching FastAPI's `HTTPException` format.
///
/// All non-2xx responses from the server use this shape so that clients
/// can parse errors uniformly. The one exception is
/// [`ServerError::JobConflict`](crate::error::ServerError::JobConflict),
/// which nests a `conflicts` array inside `detail`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    /// Human-readable description of what went wrong. For validation
    /// errors this includes the specific field or constraint that failed.
    pub detail: String,
}

/// Response body for mutating operations that return a status confirmation
/// (e.g. cancel, delete, restart).
///
/// Returned with HTTP 200 on success. Clients can branch on `status` for
/// programmatic handling and display `message` to users.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StatusMessageResponse {
    /// Machine-readable outcome token (e.g. `"cancelled"`, `"deleted"`,
    /// `"restarted"`). Suitable for switch/match in client code.
    pub status: String,
    /// Human-readable sentence describing the action taken (e.g.
    /// `"Job abc123 has been cancelled."`).
    pub message: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::health::health,
        crate::routes::jobs::submit_job,
        crate::routes::jobs::list_jobs,
        crate::routes::jobs::get_job,
        crate::routes::jobs::get_results,
        crate::routes::jobs::get_single_result,
        crate::routes::jobs::cancel_job,
        crate::routes::jobs::delete_job,
        crate::routes::jobs::restart_job,
        crate::routes::jobs::stream_job,
        crate::routes::media_list::list_media,
        crate::routes::bug_reports::list_bug_reports,
        crate::routes::bug_reports::get_bug_report
    ),
    components(
        schemas(
            ErrorResponse,
            StatusMessageResponse,
            crate::api::FilePayload,
            crate::api::JobSubmission,
            crate::api::JobStatus,
            crate::api::FileProgressStage,
            crate::api::FileResult,
            crate::api::FileStatusEntry,
            crate::api::JobInfo,
            crate::api::JobListItem,
            crate::api::JobResultResponse,
            crate::api::HealthResponse
        )
    ),
    tags(
        (name = "health", description = "Health and readiness endpoints"),
        (name = "jobs", description = "Job submission and lifecycle endpoints"),
        (name = "media", description = "Media bank and file discovery endpoints"),
        (name = "bug-reports", description = "Bug report retrieval endpoints")
    )
)]
/// Utoipa-generated OpenAPI 3.0 document for the batchalign3 server API.
pub struct ApiDoc;

fn canonicalize_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (_, child) in &mut entries {
                canonicalize_json(child);
            }
            *map = entries.into_iter().collect::<Map<String, Value>>();
        }
        Value::Array(items) => {
            for child in items {
                canonicalize_json(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Render the OpenAPI schema as pretty JSON.
pub fn openapi_json_pretty() -> Result<String, serde_json::Error> {
    let mut value = serde_json::to_value(ApiDoc::openapi())?;
    canonicalize_json(&mut value);
    serde_json::to_string_pretty(&value)
}

/// Write the OpenAPI schema to a path, creating parent directories as needed.
pub fn write_openapi_json(path: &Path) -> std::io::Result<()> {
    let json = openapi_json_pretty().map_err(std::io::Error::other)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json)
}

/// Check whether a path contains the current canonical OpenAPI schema.
pub fn check_openapi_json(path: &Path) -> std::io::Result<()> {
    let generated = openapi_json_pretty().map_err(std::io::Error::other)?;
    let existing = std::fs::read_to_string(path).map_err(|err| {
        std::io::Error::new(
            err.kind(),
            format!("failed to read OpenAPI schema {}: {err}", path.display()),
        )
    })?;

    if existing == generated {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "OpenAPI schema is out of date at {}. Regenerate with: cargo run -q -p batchalign-bin -- openapi --output {}",
            path.display(),
            path.display()
        )))
    }
}
