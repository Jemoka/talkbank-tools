//! `GET /media/list` — list available media files.
//!
//! Provides a browsable listing of audio/video files on the server's
//! configured media volumes.  The CLI uses this so users can discover
//! remote media before submitting a job that references files by name.
//! Results come from the `MediaResolver` walk cache (60 s TTL) for
//! efficient repeated access, even over NFS mounts.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::AppState;
use crate::error::ServerError;

/// Build the media listing router (`GET /media/list`).
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/media/list", get(list_media))
}

/// Query parameters for the media listing endpoint.
///
/// If `bank` is provided, the listing is restricted to a single named media
/// mapping (e.g., `"childes"` -> `/data/media/childes/`). Otherwise the
/// listing searches all configured `media_roots`. In both cases `subdir`
/// narrows the search to a subdirectory, with path-traversal protection.
#[derive(Deserialize)]
pub(crate) struct MediaListQuery {
    #[serde(default)]
    bank: String,
    #[serde(default)]
    subdir: String,
}

/// List audio/video files available on this server's configured media volumes.
///
/// The CLI uses this to let users browse remote media before submitting a job
/// that references files by name (media_files field in `JobSubmission`). Results
/// come from the `MediaResolver` walk cache (60 s TTL) so repeated calls are
/// cheap even over NFS mounts.
#[utoipa::path(
    get,
    path = "/media/list",
    tag = "media",
    params(
        ("bank" = String, Query, description = "Optional media bank key"),
        ("subdir" = String, Query, description = "Optional subdirectory under bank/root")
    ),
    responses(
        (status = 200, description = "Media listing"),
        (status = 400, description = "Invalid bank/subdir", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn list_media(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MediaListQuery>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let config = &state.environment.config;

    let files = if !query.bank.is_empty() {
        let mapping_root = config
            .media_mappings
            .get(&batchalign_types::paths::MediaMappingKey::new(&query.bank))
            .ok_or_else(|| {
                ServerError::Validation(format!(
                    "Unknown media bank '{}'. Available: {:?}",
                    query.bank,
                    config.media_mappings.keys().collect::<Vec<_>>()
                ))
            })?;
        state
            .environment
            .media
            .list_mapped(mapping_root.as_str(), &query.subdir)
    } else {
        state.environment.media.list_files(
            &config
                .media_roots
                .iter()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<_>>(),
            &query.subdir,
        )
    };

    Ok(Json(serde_json::json!({ "files": files })))
}
