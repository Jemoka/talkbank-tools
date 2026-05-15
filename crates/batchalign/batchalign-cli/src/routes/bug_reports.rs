//! Bug report endpoints — `GET /bug-reports` and `GET /bug-reports/{id}`.
//!
//! Reads JSON files from the bug-reports directory (`~/.batchalign3/bug-reports/`
//! by default, configurable via `AppState.environment.paths.bug_reports_dir`).

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::AppState;
use crate::error::ServerError;

/// Query parameters for `GET /bug-reports`.
///
/// `limit` caps the number of reports returned (newest first). The default
/// of 50 keeps the response lightweight for the dashboard while still
/// surfacing recent issues.
#[derive(serde::Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Build the bug-reports router (`GET /bug-reports`, `GET /bug-reports/{id}`).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/bug-reports", get(list_bug_reports))
        .route("/bug-reports/{id}", get(get_bug_report))
}

/// Return the most recent bug reports, sorted newest-first.
///
/// Bug reports are JSON files written to disk by the validation layer when
/// it detects semantic errors (e.g., alignment mismatches, monotonicity
/// violations). This endpoint scans the reports directory by mtime so the
/// dashboard can surface recent failures without requiring database queries.
#[utoipa::path(
    get,
    path = "/bug-reports",
    tag = "bug-reports",
    params(
        ("limit" = usize, Query, description = "Maximum number of reports to return")
    ),
    responses(
        (status = 200, description = "Bug report documents (newest first)"),
        (status = 500, description = "Bug report storage could not be read", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn list_bug_reports(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<serde_json::Value>>, ServerError> {
    let dir = &state.environment.paths.bug_reports_dir;

    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|error| bug_report_io_error("read bug reports directory", dir, error))?;

    // Collect .json files with metadata for sorting
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
    loop {
        let entry = entries
            .next_entry()
            .await
            .map_err(|error| bug_report_io_error("scan bug reports directory", dir, error))?;
        let Some(entry) = entry else {
            break;
        };
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let mtime = entry
                .metadata()
                .await
                .map_err(|error| bug_report_io_error("read bug report metadata", &path, error))?
                .modified()
                .map_err(|error| {
                    bug_report_io_error("read bug report modified time", &path, error)
                })?;
            files.push((mtime, path));
        }
    }

    // Sort by mtime descending (newest first)
    files.sort_by_key(|b| std::cmp::Reverse(b.0));
    files.truncate(params.limit);

    let mut reports = Vec::new();
    for (_mtime, path) in &files {
        let report = read_bug_report(path).await?;
        reports.push(report);
    }

    Ok(Json(reports))
}

/// Return a single bug report by its ID (filename stem).
///
/// The ID corresponds to the JSON filename on disk without the `.json`
/// extension. Returns the raw JSON document as-is so the dashboard can
/// render full diagnostic detail (stack traces, file paths, expected vs.
/// actual values).
#[utoipa::path(
    get,
    path = "/bug-reports/{id}",
    tag = "bug-reports",
    params(
        ("id" = String, Path, description = "Bug report ID")
    ),
    responses(
        (status = 200, description = "Bug report JSON document"),
        (status = 404, description = "Bug report not found", body = crate::openapi::ErrorResponse),
        (status = 500, description = "Bug report storage could not be read", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn get_bug_report(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let path =
        std::path::Path::new(&state.environment.paths.bug_reports_dir).join(format!("{id}.json"));

    let report = match read_bug_report(&path).await {
        Ok(report) => report,
        Err(ServerError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ServerError::FileNotFound(format!(
                "Bug report {id} not found"
            )));
        }
        Err(error) => return Err(error),
    };

    Ok(Json(report))
}

async fn read_bug_report(path: &std::path::Path) -> Result<serde_json::Value, ServerError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| bug_report_io_error("read bug report", path, error))?;

    serde_json::from_str(&content).map_err(|error| {
        ServerError::Persistence(format!(
            "invalid bug report JSON at {}: {error}",
            path.display()
        ))
    })
}

fn bug_report_io_error(
    action: &str,
    path: impl AsRef<std::path::Path>,
    error: std::io::Error,
) -> ServerError {
    let path = path.as_ref();
    ServerError::Io(std::io::Error::new(
        error.kind(),
        format!("{action} {}: {error}", path.display()),
    ))
}
