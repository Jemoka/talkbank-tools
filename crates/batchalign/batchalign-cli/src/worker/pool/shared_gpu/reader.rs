//! Generic response reader loop shared between stdio and TCP GPU workers.
//!
//! Reads JSON-lines from any `AsyncBufRead`, routing V2 responses by
//! `request_id` to pending oneshot senders, and non-V2 responses
//! (health, capabilities, shutdown, error) to a control channel.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::oneshot;
use tracing::{debug, error, warn};

use tokio::io::AsyncBufReadExt;

use crate::types::worker_v2::ExecuteResponseV2;
use crate::worker::WorkerPid;

use super::WorkerControlResponse;
use super::envelopes::{
    CapabilitiesResponseEnvelope, EnsureTaskResponseEnvelope, ExecuteResponseV2Envelope,
    HealthResponseEnvelope,
};

/// Generic reader loop that works with any `AsyncBufRead` — shared between
/// stdio ([`super::SharedGpuWorker`]) and TCP ([`super::SharedGpuTcpWorker`]).
pub(crate) async fn reader_loop_generic<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    pending: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<ExecuteResponseV2>>>>,
    control: Arc<tokio::sync::Mutex<Option<oneshot::Sender<WorkerControlResponse>>>>,
    pid: WorkerPid,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!(pid = %pid, "GPU worker stream closed (EOF)");
                let mut pending = super::super::lock_recovered(&pending);
                for (id, tx) in pending.drain() {
                    debug!(pid = %pid, request_id = %id, "Failing pending request (worker stream closed)");
                    drop(tx);
                }
                break;
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let parsed: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(
                            pid = %pid,
                            line = trimmed,
                            error = %e,
                            "GPU worker: ignoring non-JSON line"
                        );
                        continue;
                    }
                };

                let op = parsed.get("op").and_then(|v| v.as_str()).unwrap_or("");

                match op {
                    "execute_v2" => {
                        match serde_json::from_value::<ExecuteResponseV2Envelope>(parsed.clone()) {
                            Ok(envelope) => {
                                let request_id = envelope.response.request_id.to_string();
                                let mut pending = super::super::lock_recovered(&pending);
                                if let Some(tx) = pending.remove(&request_id) {
                                    let _ = tx.send(envelope.response);
                                } else {
                                    warn!(
                                        pid = %pid,
                                        request_id = %request_id,
                                        "GPU worker: orphaned execute_v2 response"
                                    );
                                }
                            }
                            Err(e) => {
                                error!(
                                    pid = %pid,
                                    error = %e,
                                    "GPU worker: failed to parse execute_v2 response"
                                );
                            }
                        }
                    }
                    "health" => {
                        if let Ok(envelope) =
                            serde_json::from_value::<HealthResponseEnvelope>(parsed)
                        {
                            let mut ctrl = control.lock().await;
                            if let Some(tx) = ctrl.take() {
                                let _ = tx.send(WorkerControlResponse::Health(envelope.response));
                            }
                        }
                    }
                    "capabilities" => {
                        if let Ok(envelope) =
                            serde_json::from_value::<CapabilitiesResponseEnvelope>(parsed)
                        {
                            let mut ctrl = control.lock().await;
                            if let Some(tx) = ctrl.take() {
                                let _ =
                                    tx.send(WorkerControlResponse::Capabilities(envelope.response));
                            }
                        }
                    }
                    "ensure_task" => {
                        if let Ok(envelope) =
                            serde_json::from_value::<EnsureTaskResponseEnvelope>(parsed)
                        {
                            let mut ctrl = control.lock().await;
                            if let Some(tx) = ctrl.take() {
                                let _ =
                                    tx.send(WorkerControlResponse::EnsureTask(envelope.response));
                            }
                        }
                    }
                    "shutdown" => {
                        let mut ctrl = control.lock().await;
                        if let Some(tx) = ctrl.take() {
                            let _ = tx.send(WorkerControlResponse::Shutdown);
                        }
                    }
                    "error" => {
                        let error_msg = parsed
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        let mut ctrl = control.lock().await;
                        if let Some(tx) = ctrl.take() {
                            let _ = tx.send(WorkerControlResponse::Error(error_msg));
                        }
                    }
                    _ => {
                        warn!(
                            pid = %pid,
                            op = op,
                            "GPU worker: unexpected response op"
                        );
                    }
                }
            }
            Err(e) => {
                error!(pid = %pid, error = %e, "GPU worker: stream read error");
                // Explicitly fail all pending requests — same as the EOF
                // path. Without this, pending oneshot senders are implicitly
                // dropped when the task exits, causing receivers to see
                // "channel closed" with no useful error context.
                let mut pending = super::super::lock_recovered(&pending);
                let n = pending.len();
                for (id, tx) in pending.drain() {
                    debug!(pid = %pid, request_id = %id, "Failing pending request (I/O error)");
                    drop(tx);
                }
                if n > 0 {
                    error!(
                        pid = %pid,
                        failed_requests = n,
                        error = %e,
                        "GPU worker crashed — failed {n} pending requests"
                    );
                }
                break;
            }
        }
    }
}
