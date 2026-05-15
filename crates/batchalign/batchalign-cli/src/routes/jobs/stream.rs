//! SSE streaming endpoint for real-time job progress.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use crate::api::{JobInfo, JobStatus};
use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use tokio_stream::StreamExt;
use tracing::warn;

use crate::AppState;
use crate::error::ServerError;
use crate::ws::WsEvent;

/// SSE stream for real-time per-file progress of a single job.
///
/// Sends:
/// - `snapshot` event with current file statuses on connect
/// - `file_update` events as files are processed
/// - `complete` event when the job reaches a terminal state, then closes
#[utoipa::path(
    get,
    path = "/jobs/{job_id}/stream",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    responses(
        (status = 200, description = "SSE event stream"),
        (status = 404, description = "Job not found", body = crate::openapi::ErrorResponse)
    )
)]
pub(crate) async fn stream_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ServerError> {
    // Validate job exists
    let job_id = crate::api::JobId::from(job_id);
    let initial_info = state
        .control
        .backend
        .get_job(&job_id)
        .await
        .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))?;

    // Subscribe to broadcast BEFORE building the snapshot to avoid missing events.
    let rx = state.control.backend.subscribe_events();

    let stream = async_stream(rx, job_id, initial_info);

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

/// Build the SSE stream from a broadcast receiver.
fn async_stream(
    rx: tokio::sync::broadcast::Receiver<WsEvent>,
    job_id: crate::api::JobId,
    initial_info: JobInfo,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    // Use BroadcastStream to convert the receiver into a Stream.
    let broadcast_stream = tokio_stream::wrappers::BroadcastStream::new(rx);

    // Chain: initial snapshot event, then filtered broadcast events.
    let snapshot_data = serde_json::json!({
        "job_id": initial_info.job_id,
        "status": initial_info.status,
        "file_statuses": initial_info.file_statuses,
        "completed_files": initial_info.completed_files,
        "total_files": initial_info.total_files,
    });
    let snapshot_event: Result<Event, Infallible> = Ok(build_json_event("snapshot", snapshot_data));

    // Check if the job is already terminal — if so, send snapshot + complete and close.
    let already_terminal = initial_info.status.is_terminal();

    let initial = tokio_stream::once(snapshot_event);

    if already_terminal {
        let complete_event: Result<Event, Infallible> = Ok(build_json_event(
            "complete",
            serde_json::json!({
                "job_id": initial_info.job_id,
                "status": initial_info.status,
            }),
        ));
        // Return snapshot + complete, then stop.
        let tail = tokio_stream::once(complete_event);
        // Use StreamExt to chain, then take(2) to ensure bounded.
        return EitherStream::Left(initial.chain(tail));
    }

    // Filter broadcast events for this job_id.
    let job_id_clone = job_id.clone();
    let filtered = broadcast_stream.filter_map(move |result| {
        let event = match result {
            Ok(event) => event,
            Err(_) => return None, // Lagged or closed
        };

        match &event {
            WsEvent::FileUpdate {
                job_id: eid,
                file,
                completed_files,
            } if *eid == job_id_clone => {
                let data = serde_json::json!({
                    "job_id": eid,
                    "file": file,
                    "completed_files": completed_files,
                });
                Some(Ok(build_json_event("file_update", data)))
            }
            WsEvent::JobUpdate { job } => {
                let Some(event_job_id) = job.get("job_id").and_then(|v| v.as_str()) else {
                    warn!("Skipping malformed WS job_update without job_id");
                    return None;
                };
                if job_id_clone != event_job_id {
                    return None;
                }
                let Some(status_str) = job.get("status").and_then(|v| v.as_str()) else {
                    warn!(
                        job_id = event_job_id,
                        "Skipping malformed WS job_update without status"
                    );
                    return None;
                };
                let is_terminal = match status_str.parse::<JobStatus>() {
                    Ok(status) => status.is_terminal(),
                    Err(_) => {
                        warn!(
                            job_id = event_job_id,
                            status = status_str,
                            "Skipping WS job_update with invalid status"
                        );
                        return None;
                    }
                };
                if is_terminal {
                    Some(Ok(build_json_event(
                        "complete",
                        serde_json::json!({
                            "job_id": event_job_id,
                            "status": status_str,
                        }),
                    )))
                } else {
                    Some(Ok(build_json_event("job_update", job.clone())))
                }
            }
            _ => None,
        }
    });

    // The stream runs until the broadcast channel closes or the client disconnects.
    // The client closes on receiving a 'complete' event.
    EitherStream::Right(initial.chain(filtered))
}

fn build_json_event<T: serde::Serialize>(event_name: &'static str, payload: T) -> Event {
    match Event::default().event(event_name).json_data(payload) {
        Ok(event) => event,
        Err(error) => {
            warn!(event = event_name, error = %error, "Failed to serialize SSE payload");
            let detail = serde_json::to_string(&serde_json::json!({
                "detail": format!("failed to serialize {event_name} event: {error}")
            }))
            .unwrap_or_else(|_| {
                "{\"detail\":\"failed to serialize SSE error payload\"}".to_string()
            });
            Event::default().event("error").data(detail)
        }
    }
}

/// Unifies two stream types so `async_stream` can return a bounded
/// snapshot-only stream (`Left`) for already-terminal jobs or a live
/// broadcast-backed stream (`Right`) for in-progress jobs, without boxing.
enum EitherStream<L, R> {
    Left(L),
    Right(R),
}

impl<L, R, T> tokio_stream::Stream for EitherStream<L, R>
where
    L: tokio_stream::Stream<Item = T> + Unpin,
    R: tokio_stream::Stream<Item = T> + Unpin,
{
    type Item = T;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.get_mut() {
            EitherStream::Left(s) => std::pin::Pin::new(s).poll_next(cx),
            EitherStream::Right(s) => std::pin::Pin::new(s).poll_next(cx),
        }
    }
}
