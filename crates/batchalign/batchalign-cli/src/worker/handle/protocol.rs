//! Wire-level IPC protocol types and constants for Python worker communication.
//!
//! Defines the JSON-lines request/response envelopes, ready signals, and
//! diagnostic dump utilities. These types are internal to the worker handle
//! machinery — callers use the higher-level [`WorkerHandle`](super::WorkerHandle)
//! methods.

use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2, ProgressEventV2};
use crate::worker::{
    BatchInferRequest, BatchInferResponse, InferRequest, InferResponse, WorkerCapabilities,
    WorkerHealthResponse, WorkerPid,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::worker::error::WorkerError;

/// Maximum chars of startup stderr to include in error messages.
pub(super) const STARTUP_STDERR_TAIL_CHARS: usize = 2_000;

/// Maximum non-JSON preamble lines to tolerate before the ready signal.
pub(super) const MAX_READY_STDOUT_PREAMBLE_LINES: usize = 32;

/// Maximum non-protocol stdout lines to tolerate while waiting for a response.
pub(super) const MAX_RESPONSE_STDOUT_NOISE_LINES: usize = 8;

/// Maximum bytes of response/error text to include in a failure dump.
const FAILED_REQUEST_DUMP_MAX_RESPONSE_BYTES: usize = 1_024 * 1_024;

/// Ready signal emitted by the Python worker on stdout.
#[derive(Debug, Deserialize)]
pub(super) struct ReadySignal {
    pub ready: bool,
    pub pid: u32,
    pub transport: Option<String>,
}

/// TCP ready signal from stderr: `{"ready": true, "pid": N, "transport": "tcp", "port": P}`.
#[derive(Debug, Deserialize)]
pub(super) struct TcpReadySignal {
    pub ready: bool,
    pub pid: u32,
    #[allow(dead_code)]
    pub transport: Option<String>,
    pub port: Option<u16>,
}

/// Internal wire-level request envelope sent to Python.
#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum WorkerRequest<'a> {
    Infer { request: &'a InferRequest },
    BatchInfer { request: &'a BatchInferRequest },
    ExecuteV2 { request: &'a ExecuteRequestV2 },
    EnsureTask { request: EnsureTaskRequest },
    Health,
    Capabilities,
    Shutdown,
}

/// Request payload for the `ensure_task` IPC operation.
#[derive(Debug, Serialize)]
pub(super) struct EnsureTaskRequest {
    pub(super) task: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) engine_overrides: Option<std::collections::BTreeMap<String, String>>,
}

/// Internal wire-level response envelope read from Python.
///
/// The `progress_v2` variant carries intermediate progress events emitted by
/// long-running V2 tasks.  Workers emit zero or more progress lines before the
/// final `execute_v2` response.  See `execute_v2_with_progress` for the
/// multiplexed read loop.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum WorkerResponse {
    Infer {
        response: InferResponse,
    },
    BatchInfer {
        response: BatchInferResponse,
    },
    ExecuteV2 {
        response: ExecuteResponseV2,
    },
    ProgressV2 {
        event: ProgressEventV2,
    },
    EnsureTask {
        response: crate::worker::EnsureTaskResponse,
    },
    Health {
        response: WorkerHealthResponse,
    },
    Capabilities {
        response: WorkerCapabilities,
    },
    Shutdown,
    Error {
        error: String,
        /// Distinguishes deterministic bootstrap-class failures from
        /// per-request runtime failures. Optional: legacy workers that don't
        /// emit the field default to [`WorkerErrorKind::Runtime`], preserving
        /// existing retry behavior. Bootstrap-kind errors classify as
        /// terminal; see [`WorkerErrorKind`] for details.
        #[serde(default)]
        kind: WorkerErrorKind,
    },
}

/// Discriminator for worker error responses on the wire.
///
/// Workers emit ``"runtime"`` for per-request failures (the default —
/// retryable) and ``"bootstrap"`` for deterministic model-load /
/// catalog-download / package-import failures (terminal — retrying with
/// the same configuration produces the same failure).
///
/// Surfaced from the JSON wire as the optional ``kind`` field on
/// ``{"op": "error", ...}`` responses. The default of ``Runtime`` is
/// load-bearing for backward compatibility with workers that don't yet
/// emit the field; do not change the default without coordinating a
/// matching Python-side rollout.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum WorkerErrorKind {
    /// Per-request failure: retry MAY succeed (different inputs, transient
    /// resource state). The default for backward compatibility.
    #[default]
    Runtime,
    /// Deterministic bootstrap failure: model loading, catalog download,
    /// import error, missing-model-with-offline-mode, etc. Retrying the
    /// same worker with the same configuration produces the same failure.
    Bootstrap,
}

impl WorkerErrorKind {
    /// Convert the wire kind into a typed [`WorkerError`].
    pub(crate) fn into_worker_error(self, message: String) -> WorkerError {
        match self {
            WorkerErrorKind::Runtime => WorkerError::WorkerResponse(message),
            WorkerErrorKind::Bootstrap => WorkerError::Bootstrap(message),
        }
    }
}

/// Dump a failed worker IPC request to the always-on debug directory.
///
/// Writes to `~/.batchalign3/debug/failed_ipc_{timestamp}.json` so the
/// operator can inspect exactly what was sent, what came back (or didn't),
/// and which worker handled it — without needing `--debug-dir`.
///
/// The response field is truncated to [`FAILED_REQUEST_DUMP_MAX_RESPONSE_BYTES`]
/// to avoid disk exhaustion from malformed worker output.
pub(super) fn dump_failed_ipc_request(
    worker_pid: WorkerPid,
    worker_label: &str,
    request_json: &str,
    error: &WorkerError,
    response_fragment: Option<&str>,
) {
    let fallback_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(".batchalign3")
        .join("debug");
    if std::fs::create_dir_all(&fallback_dir).is_err() {
        return;
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S%3f");
    let path = fallback_dir.join(format!("failed_ipc_{timestamp}.json"));

    let truncated_response = response_fragment.map(|r| {
        if r.len() > FAILED_REQUEST_DUMP_MAX_RESPONSE_BYTES {
            format!(
                "{}... [truncated, {} bytes total]",
                &r[..FAILED_REQUEST_DUMP_MAX_RESPONSE_BYTES],
                r.len()
            )
        } else {
            r.to_string()
        }
    });

    let dump = serde_json::json!({
        "timestamp": timestamp.to_string(),
        "worker_pid": *worker_pid,
        "worker_label": worker_label,
        "error_type": format!("{error:?}").split('(').next().unwrap_or("Unknown"),
        "error_message": error.to_string(),
        "request": request_json,
        "response_fragment": truncated_response,
    });

    match serde_json::to_string_pretty(&dump) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                debug!(%e, "failed to write IPC failure dump");
            } else {
                warn!(
                    path = %path.display(),
                    worker_pid = *worker_pid,
                    "Worker IPC failure dump written for post-mortem"
                );
            }
        }
        Err(e) => debug!(%e, "failed to serialize IPC failure dump"),
    }
}
