//! `SharedGpuWorker` — concurrent V2 dispatch to a single GPU worker over stdio.
//!
//! Unlike [`CheckedOutWorker`](super::super::CheckedOutWorker) which grants
//! exclusive access via a semaphore, `SharedGpuWorker` allows multiple
//! concurrent V2 requests to one worker. The Python side runs a
//! `ThreadPoolExecutor` so GPU inference (which releases the GIL) runs in
//! parallel, sharing the same loaded models in-process.
//!
//! A background reader task continuously reads JSON-lines from worker stdout.
//! Each `ExecuteResponseV2` carries a `request_id` that maps back to a pending
//! `oneshot::Sender`. Non-V2 responses (health, capabilities, shutdown) are
//! routed via a separate control channel that serializes sequential ops.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{Semaphore, oneshot};
use tracing::{debug, info, warn};

use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2};
use crate::worker::WorkerPid;
use crate::worker::error::WorkerError;
use crate::worker::handle::{WorkerConfig, WorkerHandle};

use super::WorkerControlResponse;

/// A GPU worker that supports concurrent V2 request dispatch.
///
/// Created from a [`WorkerHandle`] by consuming its stdio channels and setting
/// up a background response router. The worker process itself runs Python's
/// `_serve_stdio_concurrent()` which dispatches requests to a thread pool.
pub(crate) struct SharedGpuWorker {
    /// Owned child process handle. Unlike the TCP shared worker, the stdio
    /// variant is the lifecycle owner and must supervise shutdown/kill.
    child: tokio::sync::Mutex<Option<Child>>,

    /// Serialized writes to worker stdin. Multiple async tasks may send
    /// requests concurrently; the mutex ensures JSON lines don't interleave.
    stdin: tokio::sync::Mutex<ChildStdin>,

    /// Pending V2 requests awaiting responses, keyed by request_id.
    /// Shared with the background reader task via `Arc`.
    pending: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<ExecuteResponseV2>>>>,

    /// Control channel for sequential non-V2 ops (health, capabilities, shutdown).
    /// Only one sequential op can be in-flight at a time.
    /// Shared with the background reader task via `Arc`.
    #[allow(dead_code)]
    control: Arc<tokio::sync::Mutex<Option<oneshot::Sender<WorkerControlResponse>>>>,

    /// Background stdout reader task handle.
    reader_task: tokio::task::JoinHandle<()>,

    /// Worker process ID.
    pid: WorkerPid,

    /// Worker configuration (for logs and restarts).
    config: WorkerConfig,

    /// Prevents new requests once shutdown starts and serializes lifecycle
    /// teardown across explicit shutdown and Drop.
    shutdown_started: AtomicBool,

    /// Rust-side cache of tasks known to be loaded in this worker.
    ///
    /// Populated after a successful `ensure_task` IPC response. Subsequent
    /// calls for the same task skip the IPC round-trip entirely. This both
    /// eliminates per-dispatch overhead for the common case AND prevents
    /// control channel contention when concurrent callers try to `ensure_task`
    /// simultaneously (the control channel is single-slot).
    loaded_tasks: tokio::sync::Mutex<HashSet<String>>,

    /// Bounds in-flight `execute_v2` calls to the Python worker's
    /// `ThreadPoolExecutor` capacity (`gpu_thread_pool_size`).
    ///
    /// Why this exists: the Python side serves V2 requests through a
    /// `ThreadPoolExecutor(max_workers=gpu_thread_pool_size)`. Without a
    /// matching Rust-side gate, more callers can register pending oneshots
    /// than Python can simultaneously serve, so late callers spend their
    /// per-request timeout budget waiting in Python's executor queue
    /// instead of doing real work. That produced the production
    /// `worker protocol error: timeout (1800s) waiting for GPU execute_v2
    /// response` cascade on `brian` (Malayalam corpus, 2026-04-25) and is
    /// pinned by
    /// `tests/gpu_concurrent_dispatch.rs::gpu_concurrent_dispatch_does_not_charge_queue_wait_against_per_request_timeout`.
    ///
    /// Permit acquisition happens **before** `pending.insert()` and the
    /// `tokio::time::timeout` wrap, so the per-request timer ticks only
    /// during work that has actually been issued to the worker.
    dispatch_semaphore: Semaphore,
}

impl SharedGpuWorker {
    /// Create a shared GPU worker from an existing [`WorkerHandle`].
    ///
    /// Consumes the handle's stdin/stdout and spawns a background reader task
    /// for response routing. The handle's `Drop` impl is bypassed — this
    /// struct takes ownership of the child process lifecycle.
    pub(in crate::worker::pool) async fn from_handle(handle: WorkerHandle) -> Self {
        let pid = handle.pid();
        let config = handle.config().clone();

        // Decompose the handle into its parts. We use into_parts() to bypass
        // the Drop impl (which would kill the child process).
        let parts = handle.into_parts();

        let stdin = tokio::sync::Mutex::new(parts.stdin);
        let child = tokio::sync::Mutex::new(Some(parts.child));
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
            Self::reader_loop(parts.stdout, reader_pending, reader_control, reader_pid).await;
        });

        let dispatch_semaphore = Semaphore::new(super::dispatch_permits_from(
            config.runtime.gpu_thread_pool_size,
        ));

        Self {
            child,
            stdin,
            pending,
            control,
            reader_task,
            pid,
            config,
            shutdown_started: AtomicBool::new(false),
            loaded_tasks: tokio::sync::Mutex::new(HashSet::new()),
            dispatch_semaphore,
        }
    }

    /// Send one typed V2 execute request and await the response.
    ///
    /// Multiple callers can invoke this concurrently up to the Python
    /// worker's `ThreadPoolExecutor` capacity (`gpu_thread_pool_size`).
    /// `dispatch_semaphore` enforces that ceiling on the Rust side **before**
    /// the per-request timeout clock starts, so a caller that has to wait
    /// for an executor slot does not spend its own budget on queue-wait
    /// behind earlier requests. Stdin writes remain serialized by `stdin`.
    pub(in crate::worker::pool) async fn execute_v2(
        &self,
        request: &ExecuteRequestV2,
    ) -> Result<ExecuteResponseV2, WorkerError> {
        if self.shutdown_started.load(Ordering::Acquire) {
            return Err(WorkerError::Protocol("GPU worker is shutting down".into()));
        }

        // Check if the reader loop is still alive. If it finished (worker
        // crashed or exited), fail fast instead of writing to a dead pipe.
        if self.reader_task.is_finished() {
            return Err(WorkerError::ProcessExited {
                code: None,
                stderr: Some("GPU worker reader loop exited — worker process is dead".into()),
            });
        }

        // Acquire one dispatch permit BEFORE registering the pending oneshot
        // and starting the per-request timer. Permit count matches the Python
        // `ThreadPoolExecutor(max_workers=gpu_thread_pool_size)` capacity, so
        // queue-wait happens here (against no timeout) instead of inside the
        // `tokio::time::timeout` wrap below. See the field-level doc on
        // `dispatch_semaphore` for the production failure this prevents.
        // The permit is released when `_permit` drops at function end.
        let _permit = self.dispatch_semaphore.acquire().await.map_err(|_| {
            WorkerError::Protocol(
                "GPU worker dispatch semaphore closed (worker shutting down)".into(),
            )
        })?;

        // Re-check shutdown after acquiring the permit — the worker may have
        // started tearing down while we waited. Without this, a caller that
        // raced shutdown could write to a closing stdin and observe a
        // confusing `ProcessExited` instead of the explicit shutdown reason.
        if self.shutdown_started.load(Ordering::Acquire) {
            return Err(WorkerError::Protocol("GPU worker is shutting down".into()));
        }

        let request_id = request.request_id.to_string();
        let (tx, rx) = oneshot::channel();

        // Register the pending response channel before writing the request,
        // so the reader task can route the response as soon as it arrives.
        {
            let mut pending = super::super::lock_recovered(&self.pending);
            pending.insert(request_id.clone(), tx);
        }

        // Write the request under the stdin mutex.
        {
            let mut stdin = self.stdin.lock().await;
            let envelope = serde_json::json!({
                "op": "execute_v2",
                "request": request
            });
            let mut line = serde_json::to_string(&envelope)
                .map_err(|e| WorkerError::Protocol(format!("failed to encode request: {e}")))?;
            line.push('\n');
            if let Err(e) = stdin.write_all(line.as_bytes()).await {
                // Remove the pending entry on write failure.
                super::super::lock_recovered(&self.pending).remove(&request_id);
                return Err(e.into());
            }
            if let Err(e) = stdin.flush().await {
                super::super::lock_recovered(&self.pending).remove(&request_id);
                return Err(e.into());
            }
        }

        // Wait for the response with a timeout.
        let timeout_s = request.timeout_seconds_with_config(
            self.config.audio_task_timeout_s,
            self.config.analysis_task_timeout_s,
        );
        let timeout = Duration::from_secs(timeout_s);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                // Sender dropped — reader loop died or worker process exited.
                // The reader loop drains pending requests on both EOF and I/O
                // errors, so this means the worker crashed.
                Err(WorkerError::ProcessExited {
                    code: None,
                    stderr: Some("GPU worker response channel closed — worker process crashed during inference".into()),
                })
            }
            Err(_) => {
                // Timeout — remove the pending entry.
                super::super::lock_recovered(&self.pending).remove(&request_id);
                Err(WorkerError::Protocol(format!(
                    "timeout ({timeout_s}s) waiting for GPU execute_v2 response (request_id={request_id})"
                )))
            }
        }
    }

    /// Run a health check via the control channel.
    #[allow(dead_code)]
    pub(in crate::worker::pool) async fn health_check(
        &self,
    ) -> Result<crate::worker::WorkerHealthResponse, WorkerError> {
        if self.shutdown_started.load(Ordering::Acquire) {
            return Err(WorkerError::HealthCheckFailed(
                "GPU worker is shutting down".into(),
            ));
        }

        let (tx, rx) = oneshot::channel();
        {
            let mut ctrl = self.control.lock().await;
            *ctrl = Some(tx);
        }

        // Write health request.
        {
            let mut stdin = self.stdin.lock().await;
            let line = b"{\"op\":\"health\"}\n";
            stdin.write_all(line).await?;
            stdin.flush().await?;
        }

        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(WorkerControlResponse::Health(response))) => {
                if !response.status.is_ok() {
                    return Err(WorkerError::HealthCheckFailed(format!(
                        "status={}",
                        response.status
                    )));
                }
                Ok(response)
            }
            Ok(Ok(WorkerControlResponse::Error(error))) => {
                Err(WorkerError::HealthCheckFailed(error))
            }
            Ok(Ok(other)) => Err(WorkerError::HealthCheckFailed(format!(
                "unexpected control response for health: {other:?}"
            ))),
            Ok(Err(_)) => Err(WorkerError::HealthCheckFailed(
                "control channel closed".into(),
            )),
            Err(_) => Err(WorkerError::HealthCheckFailed(
                "timeout waiting for health response".into(),
            )),
        }
    }

    /// Load one task's models on demand via the `ensure_task` IPC operation.
    ///
    /// Used by `LazyProfile` workers that start with no models loaded. The Rust
    /// control plane calls this before dispatching work for a task, so the worker
    /// has the right models resident. Idempotent: calling for an already-loaded
    /// task returns immediately with `status: "already_loaded"`.
    ///
    /// The timeout is generous because model loading involves downloading and
    /// initializing large neural models (Whisper, Wave2Vec, Stanza).
    pub(in crate::worker::pool) async fn ensure_task(
        &self,
        task: &str,
        engine_overrides: Option<&std::collections::BTreeMap<String, String>>,
        timeout_s: u64,
    ) -> Result<crate::worker::EnsureTaskResponse, WorkerError> {
        // Fast path: skip IPC if we already know this task is loaded.
        // This eliminates per-dispatch overhead for the common case and
        // prevents control channel contention under concurrent dispatch.
        {
            let cache = self.loaded_tasks.lock().await;
            if cache.contains(task) {
                return Ok(crate::worker::EnsureTaskResponse {
                    status: crate::worker::EnsureTaskStatus::AlreadyLoadedCached,
                    task: task.to_owned(),
                    elapsed_s: 0.0,
                });
            }
        }

        if self.shutdown_started.load(Ordering::Acquire) {
            return Err(WorkerError::Protocol(
                "GPU worker is shutting down — cannot ensure_task".into(),
            ));
        }

        let (tx, rx) = oneshot::channel();
        {
            let mut ctrl = self.control.lock().await;
            *ctrl = Some(tx);
        }

        {
            let mut stdin = self.stdin.lock().await;
            let envelope = serde_json::json!({
                "op": "ensure_task",
                "request": {
                    "task": task,
                    "engine_overrides": engine_overrides,
                }
            });
            let mut line = serde_json::to_string(&envelope).map_err(|e| {
                WorkerError::Protocol(format!("failed to encode ensure_task request: {e}"))
            })?;
            line.push('\n');
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await?;
        }

        match tokio::time::timeout(Duration::from_secs(timeout_s), rx).await {
            Ok(Ok(WorkerControlResponse::EnsureTask(response))) => {
                info!(
                    pid = %self.pid,
                    task = task,
                    status = %response.status,
                    elapsed_s = response.elapsed_s,
                    "ensure_task completed"
                );
                // Cache the loaded task so subsequent calls skip IPC.
                self.loaded_tasks.lock().await.insert(task.to_owned());
                Ok(response)
            }
            Ok(Ok(WorkerControlResponse::Error(error))) => Err(WorkerError::Protocol(format!(
                "ensure_task failed: {error}"
            ))),
            Ok(Ok(other)) => Err(WorkerError::Protocol(format!(
                "unexpected control response for ensure_task: {other:?}"
            ))),
            Ok(Err(_)) => Err(WorkerError::Protocol(
                "ensure_task: control channel closed".into(),
            )),
            Err(_) => Err(WorkerError::Protocol(format!(
                "timeout ({timeout_s}s) waiting for ensure_task({task}) response"
            ))),
        }
    }

    /// Gracefully shut down the GPU worker.
    pub(in crate::worker::pool) async fn shutdown(&self) {
        if self
            .shutdown_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        info!(
            target = %self.config.bootstrap_label(),
            pid = %self.pid,
            "Shutting down shared GPU worker"
        );

        let shutdown_ack = {
            let (tx, rx) = oneshot::channel();
            let mut ctrl = self.control.lock().await;
            if ctrl.is_some() {
                warn!(
                    pid = %self.pid,
                    "Overriding in-flight GPU worker control request during shutdown"
                );
            }
            *ctrl = Some(tx);
            rx
        };

        let wrote_shutdown = {
            let mut stdin = self.stdin.lock().await;
            match stdin.write_all(b"{\"op\":\"shutdown\"}\n").await {
                Ok(()) => match stdin.flush().await {
                    Ok(()) => true,
                    Err(error) => {
                        warn!(
                            pid = %self.pid,
                            error = %error,
                            "Failed to flush shared GPU shutdown request"
                        );
                        false
                    }
                },
                Err(error) => {
                    warn!(
                        pid = %self.pid,
                        error = %error,
                        "Failed to write shared GPU shutdown request"
                    );
                    false
                }
            }
        };

        if wrote_shutdown {
            match tokio::time::timeout(Duration::from_secs(2), shutdown_ack).await {
                Ok(Ok(WorkerControlResponse::Shutdown)) => {}
                Ok(Ok(other)) => {
                    warn!(
                        pid = %self.pid,
                        response = ?other,
                        "Shared GPU worker returned unexpected shutdown response"
                    );
                }
                Ok(Err(_)) => {
                    debug!(pid = %self.pid, "Shared GPU shutdown ack channel closed");
                }
                Err(_) => {
                    debug!(pid = %self.pid, "Timed out waiting for shared GPU shutdown ack");
                }
            }
        }

        self.finish_shutdown().await;
    }

    /// The worker process ID.
    pub(in crate::worker::pool) fn pid(&self) -> WorkerPid {
        self.pid
    }

    /// The worker's profile label.
    pub(in crate::worker::pool) fn profile_label(&self) -> String {
        self.config.bootstrap_label()
    }

    /// The worker's language code.
    pub(in crate::worker::pool) fn lang(&self) -> &str {
        self.config.lang.as_worker_arg()
    }

    /// Background reader loop that routes responses from worker stdout.
    async fn reader_loop(
        mut stdout: BufReader<ChildStdout>,
        pending: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<ExecuteResponseV2>>>>,
        control: Arc<tokio::sync::Mutex<Option<oneshot::Sender<WorkerControlResponse>>>>,
        pid: WorkerPid,
    ) {
        super::reader::reader_loop_generic(&mut stdout, pending, control, pid).await;
    }

    async fn finish_shutdown(&self) {
        // Layer 3: remove PID file before killing.
        super::super::reaper::remove_worker_pid(self.pid.0);

        let mut child = {
            let mut child_slot = self.child.lock().await;
            child_slot.take()
        };

        if let Some(mut child) = child.take() {
            #[cfg(unix)]
            {
                let _ = child.id().map(|pid| {
                    // SAFETY: the worker was spawned as its own process group.
                    unsafe { libc::killpg(pid as libc::pid_t, libc::SIGTERM) };
                });
            }

            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    info!(pid = %self.pid, ?status, "Shared GPU worker exited gracefully");
                }
                Ok(Err(error)) => {
                    warn!(pid = %self.pid, error = %error, "Error waiting for shared GPU worker");
                }
                Err(_) => {
                    warn!(
                        pid = %self.pid,
                        "Shared GPU worker didn't exit in 5s, killing process group"
                    );
                    #[cfg(unix)]
                    {
                        let _ = child.id().map(|pid| {
                            // SAFETY: the worker was spawned as its own process group.
                            unsafe { libc::killpg(pid as libc::pid_t, libc::SIGKILL) };
                        });
                    }
                    let _ = child.kill().await;
                }
            }
        }

        self.reader_task.abort();
        self.fail_pending_requests();
        let mut ctrl = self.control.lock().await;
        ctrl.take();
    }

    fn fail_pending_requests(&self) {
        let mut pending = super::super::lock_recovered(&self.pending);
        for (_, tx) in pending.drain() {
            drop(tx);
        }
    }
}

impl Drop for SharedGpuWorker {
    fn drop(&mut self) {
        self.shutdown_started.store(true, Ordering::Release);
        super::super::reaper::remove_worker_pid(self.pid.0);
        self.reader_task.abort();
        self.fail_pending_requests();

        if let Ok(mut child_slot) = self.child.try_lock()
            && let Some(child) = child_slot.as_mut()
        {
            #[cfg(unix)]
            {
                if let Some(pid) = child.id() {
                    let pgid = pid as libc::pid_t;
                    // SAFETY: the worker was spawned as its own process group.
                    unsafe {
                        libc::killpg(pgid, libc::SIGTERM);
                    }
                    // Brief pause then SIGKILL to prevent zombies holding GPU/RAM.
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    if unsafe { libc::kill(pgid, 0) } == 0 {
                        unsafe {
                            libc::killpg(pgid, libc::SIGKILL);
                        }
                    }
                }
            }
            let _ = child.start_kill();
        }
    }
}
