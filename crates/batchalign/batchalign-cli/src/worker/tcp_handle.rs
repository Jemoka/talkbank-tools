//! `TcpWorkerHandle` — connects to a pre-started TCP worker daemon.
//!
//! Unlike [`WorkerHandle`](super::handle::WorkerHandle) which spawns and owns a
//! child process, `TcpWorkerHandle` connects to a worker that is already running
//! as a persistent daemon listening on a TCP port. The same JSON-lines protocol
//! is used — the only difference is the transport layer.
//!
//! # Lifecycle
//!
//! - `TcpWorkerHandle` does **not** own the worker process. Dropping the handle
//!   disconnects the TCP stream but does not kill the worker.
//! - If the connection drops, [`reconnect()`](TcpWorkerHandle::reconnect) tries
//!   to re-establish it before failing the request.
//! - Shutdown sends the `{"op":"shutdown"}` message but does not SIGKILL — the
//!   worker daemon is managed by launchd/systemd, not Rust.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use crate::api::WorkerLanguage;
use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2, ProgressEventV2};
use crate::worker::error::WorkerError;
use crate::worker::{
    BatchInferRequest, BatchInferResponse, InferRequest, InferResponse, WorkerCapabilities,
    WorkerHealthResponse, WorkerPid, WorkerProfile,
};

/// Maximum non-JSON lines to tolerate while waiting for a response.
const MAX_RESPONSE_NOISE_LINES: usize = 8;

/// Wire-level request envelope (same as handle.rs — shared protocol).
#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum WorkerRequest<'a> {
    Infer { request: &'a InferRequest },
    BatchInfer { request: &'a BatchInferRequest },
    ExecuteV2 { request: &'a ExecuteRequestV2 },
    Health,
    Capabilities,
    Shutdown,
}

/// Wire-level response envelope (same as handle.rs — shared protocol).
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum WorkerResponse {
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
    Health {
        response: WorkerHealthResponse,
    },
    Capabilities {
        response: WorkerCapabilities,
    },
    Shutdown,
    Error {
        error: String,
        /// Bootstrap-vs-runtime discriminator; see [`WorkerErrorKind`] for
        /// the definitive doc. Default ``Runtime`` keeps existing retry
        /// semantics for legacy workers that don't emit the field.
        #[serde(default)]
        kind: crate::worker::handle::WorkerErrorKind,
    },
}

/// Metadata about a discovered TCP worker (from registry).
#[derive(Debug, Clone)]
pub struct TcpWorkerInfo {
    /// Host address (usually 127.0.0.1).
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Worker profile.
    pub profile: WorkerProfile,
    /// Worker-runtime language string.
    pub lang: WorkerLanguage,
    /// Engine overrides JSON string.
    pub engine_overrides: String,
    /// Worker process ID (from registry, for display).
    pub pid: WorkerPid,
    /// Timeout for audio-heavy tasks (ASR, FA, speaker). 0 = default (1800).
    pub audio_task_timeout_s: u64,
    /// Timeout for analysis tasks (OpenSMILE, AVQI). 0 = default (120).
    pub analysis_task_timeout_s: u64,
    /// Python worker's `ThreadPoolExecutor(max_workers=...)` capacity for
    /// concurrent V2 dispatch. Used by `SharedGpuTcpWorker` to cap in-flight
    /// `execute_v2` calls so per-request timeouts never count queue-wait
    /// behind earlier requests. Ignored by `TcpWorkerHandle` (which serves
    /// one request at a time anyway). Daemons are spawned with
    /// `--gpu-thread-pool-size`; pool callers should pass the same value
    /// they used at spawn (or registry-discovered).
    pub gpu_thread_pool_size: u32,
}

/// Manages a TCP connection to a pre-started Python worker daemon.
///
/// Uses the same JSON-lines protocol as [`WorkerHandle`] but over TCP instead
/// of stdio pipes. Does not own the worker process — dropping disconnects but
/// does not kill.
pub struct TcpWorkerHandle {
    info: TcpWorkerInfo,
    reader: BufReader<tokio::io::ReadHalf<TcpStream>>,
    writer: tokio::io::WriteHalf<TcpStream>,
    /// Monotonic instant when the last request was dispatched.
    last_activity: tokio::time::Instant,
}

impl TcpWorkerHandle {
    /// Connect to an existing TCP worker.
    pub async fn connect(info: TcpWorkerInfo) -> Result<Self, WorkerError> {
        let addr = format!("{}:{}", info.host, info.port);
        info!(
            host = %info.host,
            port = info.port,
            profile = %info.profile.label(),
            lang = %info.lang,
            pid = %info.pid,
            "Connecting to TCP worker"
        );

        let stream = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                WorkerError::Protocol(format!("timeout connecting to TCP worker at {addr}"))
            })?
            .map_err(|e| {
                WorkerError::Protocol(format!("failed to connect to TCP worker at {addr}: {e}"))
            })?;

        let (read_half, write_half) = tokio::io::split(stream);

        Ok(Self {
            info,
            reader: BufReader::new(read_half),
            writer: write_half,
            last_activity: tokio::time::Instant::now(),
        })
    }

    /// Reconnect to the worker after a connection drop.
    pub async fn reconnect(&mut self) -> Result<(), WorkerError> {
        let addr = format!("{}:{}", self.info.host, self.info.port);
        debug!(addr = %addr, "Reconnecting to TCP worker");

        let stream = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                WorkerError::Protocol(format!("timeout reconnecting to TCP worker at {addr}"))
            })?
            .map_err(|e| {
                WorkerError::Protocol(format!("failed to reconnect to TCP worker at {addr}: {e}"))
            })?;

        let (read_half, write_half) = tokio::io::split(stream);
        self.reader = BufReader::new(read_half);
        self.writer = write_half;
        Ok(())
    }

    async fn write_request(&mut self, request: &WorkerRequest<'_>) -> Result<(), WorkerError> {
        let mut line = serde_json::to_string(request)
            .map_err(|e| WorkerError::Protocol(format!("failed to encode request: {e}")))?;
        line.push('\n');

        match self.writer.write_all(line.as_bytes()).await {
            Ok(()) => {}
            Err(e) => {
                // Try reconnect once before failing.
                warn!(error = %e, "TCP write failed, attempting reconnect");
                self.reconnect().await?;
                self.writer.write_all(line.as_bytes()).await?;
            }
        }
        self.writer.flush().await?;
        Ok(())
    }

    async fn read_response(&mut self) -> Result<WorkerResponse, WorkerError> {
        let mut skipped_noise_lines = 0usize;

        loop {
            let mut line = String::new();
            let bytes = self.reader.read_line(&mut line).await?;
            if bytes == 0 {
                return Err(WorkerError::Protocol(
                    "TCP worker closed connection (EOF)".into(),
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<WorkerResponse>(&line) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if trimmed.starts_with('{') || trimmed.starts_with('[') {
                        return Err(WorkerError::Protocol(format!(
                            "failed to decode TCP response: {e} (line: {line:?})"
                        )));
                    }

                    skipped_noise_lines += 1;
                    warn!(
                        host = %self.info.host,
                        port = self.info.port,
                        line = trimmed,
                        "Ignoring non-protocol TCP line"
                    );

                    if skipped_noise_lines >= MAX_RESPONSE_NOISE_LINES {
                        return Err(WorkerError::Protocol(format!(
                            "TCP worker emitted too many non-protocol lines; last: {line:?}"
                        )));
                    }
                }
            }
        }
    }

    /// Twin of `WorkerHandle::read_response_skipping_progress_via_self`
    /// for the TCP transport. Skip every `ProgressV2` event, optionally
    /// forward to a channel, return the first non-progress response, or
    /// time out at the deadline. See `worker/handle/progress_preamble.rs`
    /// for the bug-class rationale and the unit-tested static twin.
    async fn read_response_skipping_progress(
        &mut self,
        deadline: tokio::time::Instant,
        progress_tx: Option<&tokio::sync::mpsc::Sender<ProgressEventV2>>,
    ) -> Result<WorkerResponse, WorkerError> {
        loop {
            let response = tokio::time::timeout_at(deadline, self.read_response())
                .await
                .map_err(|_| {
                    WorkerError::Protocol("timeout waiting for TCP worker response".into())
                })??;

            match response {
                WorkerResponse::ProgressV2 { event } => {
                    if let Some(tx) = progress_tx {
                        let _ = tx.try_send(event);
                    }
                    continue;
                }
                other => return Ok(other),
            }
        }
    }

    /// Check if the worker is healthy.
    pub async fn health_check(&mut self) -> Result<WorkerHealthResponse, WorkerError> {
        self.write_request(&WorkerRequest::Health).await?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let response = self
            .read_response_skipping_progress(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::HealthCheckFailed("timeout waiting for TCP health response".into())
                }
                other => other,
            })?;

        match response {
            WorkerResponse::Health { response } => {
                if !response.status.is_ok() {
                    return Err(WorkerError::HealthCheckFailed(format!(
                        "status={}",
                        response.status
                    )));
                }
                Ok(response)
            }
            WorkerResponse::Error { error, kind: _ } => Err(WorkerError::HealthCheckFailed(error)),
            other => Err(WorkerError::HealthCheckFailed(format!(
                "unexpected TCP response for health: {other:?}"
            ))),
        }
    }

    /// Send a single inference request.
    pub async fn infer(&mut self, request: &InferRequest) -> Result<InferResponse, WorkerError> {
        self.last_activity = tokio::time::Instant::now();
        self.write_request(&WorkerRequest::Infer { request })
            .await?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        let response = self
            .read_response_skipping_progress(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::Protocol("timeout waiting for TCP infer response".into())
                }
                other => other,
            })?;

        match response {
            WorkerResponse::Infer { response } => Ok(response),
            WorkerResponse::Error { error, kind } => Err(kind.into_worker_error(error)),
            other => Err(WorkerError::Protocol(format!(
                "unexpected TCP response for infer: {other:?}"
            ))),
        }
    }

    /// Send a batched inference request.
    pub async fn batch_infer(
        &mut self,
        request: &BatchInferRequest,
    ) -> Result<BatchInferResponse, WorkerError> {
        self.last_activity = tokio::time::Instant::now();
        self.write_request(&WorkerRequest::BatchInfer { request })
            .await?;

        let timeout_s = (request.items.len() as u64 * 5).max(120);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);
        let item_count = request.items.len();
        let response = self
            .read_response_skipping_progress(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::Protocol(format!(
                        "timeout ({timeout_s}s) waiting for TCP batch_infer response ({item_count} items)"
                    ))
                }
                other => other,
            })?;

        match response {
            WorkerResponse::BatchInfer { response } => Ok(response),
            WorkerResponse::Error { error, kind } => Err(kind.into_worker_error(error)),
            other => Err(WorkerError::Protocol(format!(
                "unexpected TCP response for batch_infer: {other:?}"
            ))),
        }
    }

    /// Send one typed V2 execute request.
    pub async fn execute_v2(
        &mut self,
        request: &ExecuteRequestV2,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        self.execute_v2_with_progress(request, None).await
    }

    /// Send an `execute_v2` request over TCP, forwarding intermediate
    /// progress events through an optional async channel.
    pub async fn execute_v2_with_progress(
        &mut self,
        request: &ExecuteRequestV2,
        progress_tx: Option<&tokio::sync::mpsc::Sender<ProgressEventV2>>,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        self.last_activity = tokio::time::Instant::now();
        self.write_request(&WorkerRequest::ExecuteV2 { request })
            .await?;

        let timeout_s = request.timeout_seconds_with_config(
            self.info.audio_task_timeout_s,
            self.info.analysis_task_timeout_s,
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);

        loop {
            let response = tokio::time::timeout_at(deadline, self.read_response())
                .await
                .map_err(|_| {
                    WorkerError::Protocol(format!(
                        "timeout ({timeout_s}s) waiting for TCP execute_v2 response ({:?})",
                        request.task
                    ))
                })??;

            match response {
                WorkerResponse::ProgressV2 { event } => {
                    if let Some(tx) = progress_tx {
                        let _ = tx.try_send(event);
                    }
                    continue;
                }
                WorkerResponse::ExecuteV2 { response } => return Ok(response),
                WorkerResponse::Error { error, kind } => {
                    return Err(kind.into_worker_error(error));
                }
                other => {
                    return Err(WorkerError::Protocol(format!(
                        "unexpected TCP response for execute_v2: {other:?}"
                    )));
                }
            }
        }
    }

    /// Query worker capabilities.
    pub async fn capabilities(&mut self) -> Result<WorkerCapabilities, WorkerError> {
        self.write_request(&WorkerRequest::Capabilities).await?;

        // Same protocol contract as the stdio handle: tolerate
        // progress_v2 preamble during cold-cache catalog/model loads.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let response = self
            .read_response_skipping_progress(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::Protocol("timeout waiting for TCP capabilities response".into())
                }
                other => other,
            })?;

        match response {
            WorkerResponse::Capabilities { response } => Ok(response),
            WorkerResponse::Error { error, kind } => Err(kind.into_worker_error(error)),
            other => Err(WorkerError::Protocol(format!(
                "unexpected TCP response for capabilities: {other:?}"
            ))),
        }
    }

    /// Send shutdown message (does not kill process — daemon manager handles that).
    pub async fn shutdown(&mut self) -> Result<(), WorkerError> {
        info!(
            host = %self.info.host,
            port = self.info.port,
            pid = %self.info.pid,
            "Sending shutdown to TCP worker"
        );

        let _ = self.write_request(&WorkerRequest::Shutdown).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), self.read_response()).await;
        Ok(())
    }

    /// The PID of the worker process (from registry).
    pub fn pid(&self) -> WorkerPid {
        self.info.pid
    }

    /// The profile label this worker handles.
    pub fn profile_label(&self) -> &'static str {
        self.info.profile.label()
    }

    /// The language this worker handles.
    pub fn lang(&self) -> &str {
        self.info.lang.as_worker_arg()
    }

    /// The transport this worker uses.
    pub fn transport(&self) -> &'static str {
        "tcp"
    }

    /// Duration since the last request was dispatched.
    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// The TCP connection info.
    pub fn info(&self) -> &TcpWorkerInfo {
        &self.info
    }
}
