//! Request/response IPC methods for [`WorkerHandle`].
//!
//! All communication with the Python worker child process flows through
//! these methods: writing JSON-lines requests to stdin, reading JSON-lines
//! responses from stdout, and handling timeouts, noise lines, and crashes.

use std::time::Duration;

use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2, ProgressEventV2};
use crate::worker::error::WorkerError;
use crate::worker::{
    BatchInferRequest, BatchInferResponse, InferRequest, InferResponse, WorkerCapabilities,
    WorkerHealthResponse,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tracing::warn;

use super::WorkerHandle;
use super::protocol::{
    MAX_RESPONSE_STDOUT_NOISE_LINES, WorkerRequest, WorkerResponse, dump_failed_ipc_request,
};

impl WorkerHandle {
    /// Serialize and write a JSON-lines request to the worker's stdin.
    pub(super) async fn write_request(
        &mut self,
        request: &WorkerRequest<'_>,
    ) -> Result<(), WorkerError> {
        let mut line = serde_json::to_string(request)
            .map_err(|e| WorkerError::Protocol(format!("failed to encode request: {e}")))?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Read and parse a single JSON-lines response from the worker's stdout.
    ///
    /// Skips empty lines and non-JSON noise (up to
    /// [`MAX_RESPONSE_STDOUT_NOISE_LINES`]). On pipe errors or EOF, drains
    /// stderr for diagnostic output before returning the error.
    pub(super) async fn read_response(&mut self) -> Result<WorkerResponse, WorkerError> {
        let mut skipped_noise_lines = 0usize;

        loop {
            let mut line = String::new();
            let bytes = match self.stdout.read_line(&mut line).await {
                Ok(b) => b,
                Err(io_err) => {
                    // Pipe error (BrokenPipe, etc.) — worker likely crashed.
                    // Drain stderr to capture the Python traceback before
                    // returning the error. Without this, BrokenPipe errors
                    // produce no diagnostic information.
                    let stderr = self.drain_stderr_tail(50);
                    let code = self.child.try_wait().ok().flatten().and_then(|s| s.code());
                    return Err(WorkerError::ProcessExited {
                        code,
                        stderr: stderr.or_else(|| Some(format!("I/O error: {io_err}"))),
                    });
                }
            };
            if bytes == 0 {
                let code = self.child.try_wait().ok().flatten().and_then(|s| s.code());
                let stderr = self.drain_stderr_tail(50);
                return Err(WorkerError::ProcessExited { code, stderr });
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
                            "failed to decode response: {e} (line: {line:?})"
                        )));
                    }

                    skipped_noise_lines += 1;
                    warn!(
                        pid = %self.pid,
                        target = %self.config.bootstrap_label(),
                        line = trimmed,
                        skipped_noise_lines,
                        "Ignoring non-protocol stdout while waiting for worker response"
                    );

                    if skipped_noise_lines >= MAX_RESPONSE_STDOUT_NOISE_LINES {
                        return Err(WorkerError::Protocol(format!(
                            "worker emitted too many non-protocol stdout lines while waiting for response; last line: {line:?}"
                        )));
                    }
                }
            }
        }
    }

    /// Check if the worker is healthy.
    pub async fn health_check(&mut self) -> Result<WorkerHealthResponse, WorkerError> {
        self.write_request(&WorkerRequest::Health).await?;

        // Tolerate progress preamble: a worker mid-bootstrap (e.g.
        // Stanza catalog still downloading) may emit progress_v2
        // events before it can answer the health probe.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let response = self
            .read_response_skipping_progress_via_self(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::HealthCheckFailed("timeout waiting for health response".into())
                }
                other => other,
            })?;

        let resp = match response {
            WorkerResponse::Health { response } => response,
            WorkerResponse::Error { error, kind: _ } => {
                // Health-check responses don't have a "bootstrap vs runtime"
                // distinction at the application layer — any error here means
                // the worker isn't healthy.
                return Err(WorkerError::HealthCheckFailed(error));
            }
            other => {
                return Err(WorkerError::HealthCheckFailed(format!(
                    "unexpected response for health: {other:?}"
                )));
            }
        };

        if !resp.status.is_ok() {
            return Err(WorkerError::HealthCheckFailed(format!(
                "status={}",
                resp.status
            )));
        }

        Ok(resp)
    }

    /// Load one task's models on demand in a LazyProfile worker.
    ///
    /// Sends the `ensure_task` IPC message and waits for the response.
    /// Idempotent: if the task is already loaded, the worker responds instantly.
    /// Timeout is generous (120s) for model downloads + initialization.
    pub async fn ensure_task(
        &mut self,
        task: &str,
        engine_overrides: Option<&std::collections::BTreeMap<String, String>>,
        timeout_s: u64,
    ) -> Result<(), WorkerError> {
        // Fast path: skip IPC if already known-loaded.
        if self.loaded_tasks.contains(task) {
            return Ok(());
        }

        use super::protocol::EnsureTaskRequest;

        self.write_request(&WorkerRequest::EnsureTask {
            request: EnsureTaskRequest {
                task: task.to_owned(),
                engine_overrides: engine_overrides.cloned(),
            },
        })
        .await?;

        // ensure_task is THE on-demand model-loading IPC: per-language
        // Stanza pack downloads, HuggingFace model fetches, and torch
        // checkpoint warm-ups all fire progress_v2 events from inside
        // this call. Tolerate them; the timeout is per the caller's
        // task budget, not per individual progress event.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);
        let response = self
            .read_response_skipping_progress_via_self(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => WorkerError::Protocol(
                    format!("timeout ({timeout_s}s) waiting for ensure_task({task}) response"),
                ),
                other => other,
            })?;

        match response {
            WorkerResponse::EnsureTask { response } => {
                tracing::info!(
                    pid = %self.pid,
                    task = task,
                    status = %response.status,
                    elapsed_s = response.elapsed_s,
                    "ensure_task completed (sequential worker)"
                );
                self.loaded_tasks.insert(task.to_owned());
                Ok(())
            }
            WorkerResponse::Error { error, kind } => {
                // ``ensure_task`` is the on-demand model-loading IPC; any error
                // here is by definition a bootstrap-class failure regardless
                // of the wire ``kind`` field. Default to ``Bootstrap`` if the
                // worker emits ``Runtime`` (legacy or generic) so the
                // orchestrator does not retry deterministic load failures.
                match kind {
                    crate::worker::handle::WorkerErrorKind::Bootstrap => Err(
                        WorkerError::Bootstrap(format!("ensure_task failed: {error}")),
                    ),
                    crate::worker::handle::WorkerErrorKind::Runtime => Err(WorkerError::Bootstrap(
                        format!("ensure_task failed: {error}"),
                    )),
                }
            }
            other => Err(WorkerError::Protocol(format!(
                "unexpected response for ensure_task: {other:?}"
            ))),
        }
    }

    /// Send a single inference request (CHAT-divorced protocol).
    ///
    /// The server owns all CHAT operations; this sends only structured
    /// payloads (words, lang) and receives structured results (mor, gra).
    pub async fn infer(&mut self, request: &InferRequest) -> Result<InferResponse, WorkerError> {
        self.last_activity = tokio::time::Instant::now();

        self.write_request(&WorkerRequest::Infer { request })
            .await?;

        // First-touch model loads (HF download, torch warmup) can fire
        // progress_v2 from inside the inference call too. Skip them.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        let response = self
            .read_response_skipping_progress_via_self(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::Protocol("timeout waiting for infer response".into())
                }
                other => other,
            })?;

        match response {
            WorkerResponse::Infer { response } => Ok(response),
            WorkerResponse::Error { error, kind } => Err(kind.into_worker_error(error)),
            other => Err(WorkerError::Protocol(format!(
                "unexpected response for infer: {other:?}"
            ))),
        }
    }

    /// Send a batched inference request (multiple items, one model call).
    ///
    /// Pools multiple utterances into a single NLP call for efficiency.
    pub async fn batch_infer(
        &mut self,
        request: &BatchInferRequest,
    ) -> Result<BatchInferResponse, WorkerError> {
        self.last_activity = tokio::time::Instant::now();

        self.write_request(&WorkerRequest::BatchInfer { request })
            .await?;

        // Generous timeout: roughly 5s per item, minimum 120s.
        let timeout_s = (request.items.len() as u64 * 5).max(120);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);
        let item_count = request.items.len();
        let response = self
            .read_response_skipping_progress_via_self(deadline, None)
            .await
            .map_err(|e| match e {
                WorkerError::Protocol(msg) if msg.contains("timeout") => {
                    WorkerError::Protocol(format!(
                        "timeout ({timeout_s}s) waiting for batch_infer response ({item_count} items)"
                    ))
                }
                other => other,
            })?;

        match response {
            WorkerResponse::BatchInfer { response } => Ok(response),
            WorkerResponse::Error { error, kind } => Err(kind.into_worker_error(error)),
            other => Err(WorkerError::Protocol(format!(
                "unexpected response for batch_infer: {other:?}"
            ))),
        }
    }

    /// Send one typed worker-protocol V2 execute request.
    ///
    /// This keeps the live FA migration on the same long-lived worker process
    /// and stdio transport while replacing the request/response payload shape
    /// with the staged V2 contract.
    /// Send an `execute_v2` request and return the final response.
    ///
    /// Any `ProgressV2` events emitted by the worker before the final
    /// response are silently discarded.  Use [`execute_v2_with_progress`]
    /// to receive them.
    pub async fn execute_v2(
        &mut self,
        request: &ExecuteRequestV2,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        self.execute_v2_with_progress(request, None).await
    }

    /// Send an `execute_v2` request, forwarding intermediate progress events
    /// through an optional async channel.
    ///
    /// The worker may emit zero or more `ProgressV2` JSON lines before the
    /// final `ExecuteV2` response.  Each progress event is sent through
    /// `progress_tx` (if provided) without blocking the read loop.  If the
    /// channel is full or closed, progress events are dropped silently —
    /// losing a progress update is not an error.
    pub async fn execute_v2_with_progress(
        &mut self,
        request: &ExecuteRequestV2,
        progress_tx: Option<&tokio::sync::mpsc::Sender<ProgressEventV2>>,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        self.last_activity = tokio::time::Instant::now();

        // Capture serialized request for failure dumps (before sending).
        let request_json_for_dump = serde_json::to_string(&WorkerRequest::ExecuteV2 { request })
            .unwrap_or_else(|_| "<serialization failed>".into());

        self.write_request(&WorkerRequest::ExecuteV2 { request })
            .await?;

        let timeout_s = request.timeout_seconds_with_config(
            self.config.audio_task_timeout_s,
            self.config.analysis_task_timeout_s,
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s);

        // Read loop: consume progress events until the final response arrives.
        loop {
            let response = tokio::time::timeout_at(deadline, self.read_response())
                .await
                .map_err(|_| {
                    let err = WorkerError::Protocol(format!(
                        "timeout ({timeout_s}s) waiting for execute_v2 response ({:?})",
                        request.task
                    ));
                    dump_failed_ipc_request(
                        self.pid,
                        &self.config.bootstrap_label(),
                        &request_json_for_dump,
                        &err,
                        None,
                    );
                    err
                })??;

            match response {
                WorkerResponse::ProgressV2 { event } => {
                    // Forward to the progress channel.  Drop silently if the
                    // receiver is gone or the channel is full.
                    if let Some(tx) = progress_tx {
                        let _ = tx.try_send(event);
                    }
                    continue;
                }
                WorkerResponse::ExecuteV2 { response } => return Ok(response),
                WorkerResponse::Error { error, kind } => {
                    let err = kind.into_worker_error(error);
                    dump_failed_ipc_request(
                        self.pid,
                        &self.config.bootstrap_label(),
                        &request_json_for_dump,
                        &err,
                        None,
                    );
                    return Err(err);
                }
                other => {
                    let err = WorkerError::Protocol(format!(
                        "unexpected response for execute_v2: {other:?}"
                    ));
                    dump_failed_ipc_request(
                        self.pid,
                        &self.config.bootstrap_label(),
                        &request_json_for_dump,
                        &err,
                        Some(&format!("{other:?}")),
                    );
                    return Err(err);
                }
            }
        }
    }

    /// Query the worker's capabilities.
    pub async fn capabilities(&mut self) -> Result<WorkerCapabilities, WorkerError> {
        self.write_request(&WorkerRequest::Capabilities).await?;

        // Import probes in _capabilities() may load heavy ML libraries
        // (torch, whisper, pyannote) on first invocation, AND on first
        // run a Stanza catalog download can fire `progress_v2` events
        // before the final `capabilities` response. The 60s budget
        // allows for cold imports + initial download.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let response = self
            .read_response_skipping_progress_via_self(deadline, None)
            .await?;

        match response {
            WorkerResponse::Capabilities { response } => Ok(response),
            WorkerResponse::Error { error, kind } => Err(kind.into_worker_error(error)),
            other => Err(WorkerError::Protocol(format!(
                "unexpected response for capabilities: {other:?}"
            ))),
        }
    }
}
