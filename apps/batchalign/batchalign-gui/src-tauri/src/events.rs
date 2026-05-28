//! Daemon → frontend progress relay.
//!
//! For each active batch we open one SSE stream to
//! `GET /jobs/{job_id}/events`, parse the `progress` events (the daemon
//! writes them as `event: progress\ndata: <json>`), wrap each in
//! `ProgressV2Payload`, and emit on the `progress-v2` Tauri channel.
//!
//! The stream is owned by a tokio task; its canceller (oneshot) is
//! stored in `AppState.pumps` so the GUI can stop following a finished
//! batch via `stop_pump`.

use std::pin::Pin;
use std::time::Duration;

use eventsource_stream::Eventsource;
use futures_util::stream::StreamExt;
use tauri::{AppHandle, Emitter, Manager};

use crate::protocol::{events, ProgressV2Payload};
use crate::state::AppState;

pub async fn pump(app: AppHandle, batch_id: String, job_id: String) {
    let state = app.state::<AppState>();
    let port = match state.daemon_port() {
        Some(p) => p,
        None => return,
    };

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut pumps = state.pumps.lock().await;
        if let Some(prev) = pumps.insert(batch_id.clone(), cancel_tx) {
            let _ = prev.send(());
        }
    }

    let url = format!(
        "http://127.0.0.1:{port}/jobs/{job_id}/events",
        port = port,
        job_id = job_id,
    );
    let resp = match reqwest::Client::new()
        .get(&url)
        .timeout(Duration::from_secs(0))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = app.emit(
                events::DAEMON_FAILED,
                crate::protocol::DaemonFailedPayload {
                    reason: format!("SSE connect failed: {e}"),
                },
            );
            return;
        }
    };

    let mut stream: Pin<Box<_>> = Box::pin(resp.bytes_stream().eventsource());

    loop {
        tokio::select! {
            biased;
            _ = &mut cancel_rx => break,
            evt = stream.next() => {
                match evt {
                    Some(Ok(e)) => {
                        if e.event == "done" {
                            break;
                        }
                        if e.event != "progress" {
                            continue;
                        }
                        let parsed: serde_json::Value = match serde_json::from_str(&e.data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let _ = app.emit(
                            events::PROGRESS_V2,
                            ProgressV2Payload {
                                batch_id: batch_id.clone(),
                                job_id: job_id.clone(),
                                event: parsed,
                            },
                        );
                    }
                    Some(Err(_)) => continue,
                    None => break,
                }
            }
        }
    }

    let mut pumps = state.pumps.lock().await;
    pumps.remove(&batch_id);
}
