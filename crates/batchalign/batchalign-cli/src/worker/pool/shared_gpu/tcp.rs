//! `SharedGpuTcpWorker` — concurrent V2 dispatch to a GPU worker over TCP.
//!
//! Similar to [`SharedGpuWorker`](super::SharedGpuWorker) but connects via
//! TCP instead of stdio. Uses a background reader task to route responses by
//! `request_id`, just like the stdio variant. The key difference: dropping
//! does not kill the worker process.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::{Semaphore, oneshot};

use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2};
use crate::worker::WorkerPid;
use crate::worker::error::WorkerError;

use super::WorkerControlResponse;

/// A GPU worker that supports concurrent V2 request dispatch over TCP.
///
/// Similar to [`super::SharedGpuWorker`] but connects via TCP instead of stdio.
/// Uses a background reader task to route responses by `request_id`, just like
/// the stdio variant. The key difference: dropping does not kill the worker
/// process.
pub(crate) struct SharedGpuTcpWorker {
    /// Serialized writes to the TCP socket.
    writer: tokio::sync::Mutex<tokio::io::WriteHalf<tokio::net::TcpStream>>,

    /// Pending V2 requests awaiting responses, keyed by request_id.
    pending: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<ExecuteResponseV2>>>>,

    /// Control channel for sequential non-V2 ops.
    #[allow(dead_code)]
    control: Arc<tokio::sync::Mutex<Option<oneshot::Sender<WorkerControlResponse>>>>,

    /// Background reader task handle.
    reader_task: tokio::task::JoinHandle<()>,

    /// Worker process ID (from registry, for display).
    pid: WorkerPid,

    /// Timeout for audio-heavy tasks.
    audio_task_timeout_s: u64,

    /// Timeout for analysis tasks.
    analysis_task_timeout_s: u64,

    /// Bounds in-flight `execute_v2` calls to the daemon's
    /// `ThreadPoolExecutor` capacity (`gpu_thread_pool_size`). Mirrors the
    /// stdio variant — see `SharedGpuWorker::dispatch_semaphore` for the
    /// architectural rationale and the production failure it prevents.
    dispatch_semaphore: Semaphore,
}

impl SharedGpuTcpWorker {
    /// Connect to a TCP GPU worker and set up concurrent dispatch.
    pub(crate) async fn connect(
        info: crate::worker::tcp_handle::TcpWorkerInfo,
    ) -> Result<Self, WorkerError> {
        let addr = format!("{}:{}", info.host, info.port);
        let stream = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| {
            WorkerError::Protocol(format!("timeout connecting to TCP GPU worker at {addr}"))
        })?
        .map_err(|e| {
            WorkerError::Protocol(format!(
                "failed to connect to TCP GPU worker at {addr}: {e}"
            ))
        })?;

        let pid = info.pid;
        let audio_task_timeout_s = info.audio_task_timeout_s;
        let analysis_task_timeout_s = info.analysis_task_timeout_s;
        let dispatch_semaphore =
            Semaphore::new(super::dispatch_permits_from(info.gpu_thread_pool_size));

        let (read_half, write_half) = tokio::io::split(stream);
        let writer = tokio::sync::Mutex::new(write_half);
        let pending = Arc::new(std::sync::Mutex::new(HashMap::<
            String,
            oneshot::Sender<ExecuteResponseV2>,
        >::new()));
        let control = Arc::new(tokio::sync::Mutex::new(
            None::<oneshot::Sender<WorkerControlResponse>>,
        ));

        let reader_pending = pending.clone();
        let reader_control = control.clone();
        let reader_pid = pid;

        let reader_task = tokio::spawn(async move {
            let mut reader = BufReader::new(read_half);
            // Reuse the same reader loop logic as stdio SharedGpuWorker.
            super::reader::reader_loop_generic(
                &mut reader,
                reader_pending,
                reader_control,
                reader_pid,
            )
            .await;
        });

        Ok(Self {
            writer,
            pending,
            control,
            reader_task,
            pid,
            audio_task_timeout_s,
            analysis_task_timeout_s,
            dispatch_semaphore,
        })
    }

    /// Send one typed V2 execute request concurrently.
    ///
    /// Concurrency is capped at `gpu_thread_pool_size` (the daemon's
    /// `ThreadPoolExecutor` capacity) by `dispatch_semaphore`. The permit is
    /// acquired before the per-request timeout starts, so queue-wait does
    /// not consume the request's audio/analysis timeout budget. See
    /// `SharedGpuWorker::execute_v2` (stdio variant) for the architectural
    /// rationale.
    pub(crate) async fn execute_v2(
        &self,
        request: &ExecuteRequestV2,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        // See dispatch_semaphore field doc for rationale. Permit drops at end.
        let _permit = self.dispatch_semaphore.acquire().await.map_err(|_| {
            WorkerError::Protocol(
                "TCP GPU worker dispatch semaphore closed (worker shutting down)".into(),
            )
        })?;

        let request_id = request.request_id.to_string();
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = super::super::lock_recovered(&self.pending);
            pending.insert(request_id.clone(), tx);
        }

        {
            let mut writer = self.writer.lock().await;
            let envelope = serde_json::json!({
                "op": "execute_v2",
                "request": request
            });
            let mut line = serde_json::to_string(&envelope)
                .map_err(|e| WorkerError::Protocol(format!("failed to encode request: {e}")))?;
            line.push('\n');
            if let Err(e) = writer.write_all(line.as_bytes()).await {
                super::super::lock_recovered(&self.pending).remove(&request_id);
                return Err(e.into());
            }
            if let Err(e) = writer.flush().await {
                super::super::lock_recovered(&self.pending).remove(&request_id);
                return Err(e.into());
            }
        }

        let timeout_s = request
            .timeout_seconds_with_config(self.audio_task_timeout_s, self.analysis_task_timeout_s);
        let timeout = Duration::from_secs(timeout_s);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(WorkerError::Protocol(
                "TCP GPU worker response channel closed (worker may have exited)".into(),
            )),
            Err(_) => {
                super::super::lock_recovered(&self.pending).remove(&request_id);
                Err(WorkerError::Protocol(format!(
                    "timeout ({timeout_s}s) waiting for TCP GPU execute_v2 response (request_id={request_id})"
                )))
            }
        }
    }

    /// Gracefully shut down the TCP GPU worker connection.
    pub(crate) async fn shutdown(&self) {
        {
            let mut writer = self.writer.lock().await;
            let _ = writer.write_all(b"{\"op\":\"shutdown\"}\n").await;
            let _ = writer.flush().await;
        }
        self.reader_task.abort();
        {
            let mut pending = super::super::lock_recovered(&self.pending);
            for (_, tx) in pending.drain() {
                drop(tx);
            }
        }
    }

    /// The worker process ID.
    pub(crate) fn pid(&self) -> WorkerPid {
        self.pid
    }
}
