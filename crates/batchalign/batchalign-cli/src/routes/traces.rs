//! Trace retrieval endpoints for algorithm visualization.
//!
//! Serves pre-collected algorithm traces (DP alignment, ASR pipeline, FA
//! timeline, retokenization) for completed jobs where `debug_traces` was
//! enabled at submission time.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;

use crate::AppState;
use crate::api::JobId;

/// Build the traces router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/jobs/{job_id}/traces", get(get_job_traces))
        .route("/jobs/{job_id}/traces/{file_index}", get(get_file_traces))
}

/// Retrieve all algorithm traces for a completed job.
///
/// Returns 200 with `JobTraces` if traces are available, 204 if the job
/// exists but no traces were collected (debug_traces was off or job hasn't
/// completed), or 404 if the job is unknown.
async fn get_job_traces(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let job_id = JobId::from(job_id);
    // Verify the job exists
    let _job = state
        .control
        .backend
        .get_job(&job_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Look up traces from the moka store
    match state.control.backend.get_job_traces(&job_id).await {
        Some(traces) => Ok(Json((*traces).clone()).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

/// Retrieve algorithm traces for a single file within a job.
///
/// Returns 200 with `FileTraces` if available, 204 if no traces, or 404.
async fn get_file_traces(
    State(state): State<Arc<AppState>>,
    Path((job_id, file_index)): Path<(String, usize)>,
) -> Result<impl IntoResponse, StatusCode> {
    let job_id = JobId::from(job_id);
    let _job = state
        .control
        .backend
        .get_job(&job_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    match state.control.backend.get_job_traces(&job_id).await {
        Some(traces) => match traces.files.get(&file_index) {
            Some(file_traces) => Ok(Json(file_traces.clone()).into_response()),
            None => Ok(StatusCode::NOT_FOUND.into_response()),
        },
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}
