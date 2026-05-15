//! Error types for Python worker process management.
//!
//! [`WorkerError`] covers every failure mode in the worker lifecycle: spawning
//! the child process, waiting for its ready signal, communicating over the
//! stdio JSON-lines protocol, health checking, and unexpected exits.
//!
//! # Retryability
//!
//! Some variants are retryable (the pool's health loop will automatically
//! restart dead workers), while others indicate a configuration or
//! environmental problem that requires operator intervention. Each variant's
//! doc comment notes which category it falls into.

/// Format the `ProcessExited` Display output, including stderr when available.
fn format_process_exited(code: Option<i32>, stderr: Option<&str>) -> String {
    let header = format!("worker process exited unexpectedly (exit code: {code:?})");
    match stderr {
        Some(text) if !text.is_empty() => {
            format!("{header}\n--- worker stderr ---\n{text}")
        }
        _ => header,
    }
}

/// Errors arising from Python worker process management.
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    /// The Python child process could not be created.
    ///
    /// Common causes: `python_path` does not exist, the `batchalign.worker`
    /// module is not importable (missing install), or OS resource limits
    /// (file descriptors, process count) have been reached.
    ///
    /// **Terminal** -- retrying with the same configuration will fail again.
    /// Callers should surface the inner message to the operator so they can
    /// fix the environment (install batchalign, adjust `python_path`, etc.).
    #[error("worker failed to start: {0}")]
    SpawnFailed(String),

    /// The worker process started but did not emit its JSON ready signal
    /// within the configured timeout.
    ///
    /// This usually means the worker is stuck loading heavy ML models (Stanza,
    /// Whisper) or the Python import itself is hanging. Increasing
    /// `ready_timeout_s` in [`super::pool::PoolConfig`] may help for
    /// legitimately slow model loads.
    ///
    /// **Retryable** -- a transient resource stall (disk I/O, network for
    /// model download) can resolve on retry. The pool will try again on the
    /// next dispatch.
    #[error("worker not ready after {timeout_s}s")]
    ReadyTimeout {
        /// Number of seconds waited before giving up.
        timeout_s: u64,
    },

    /// The worker emitted something on stdout before the ready signal, but it
    /// was not valid JSON or contained unexpected fields (e.g. `ready: false`,
    /// unsupported transport).
    ///
    /// Indicates a version mismatch between the Rust server and the Python
    /// `batchalign.worker` module, or stray print statements in Python code
    /// polluting stdout.
    ///
    /// **Terminal** -- the same worker code will produce the same bad output.
    /// Callers should log the raw line for debugging and check that the
    /// installed batchalign version matches the server.
    #[error("failed to parse ready signal: {0}")]
    ReadyParseFailed(String),

    /// A periodic health check (or an on-demand probe) failed.
    ///
    /// The worker may have become unresponsive (deadlocked, GIL contention)
    /// or returned an unhealthy status. The pool's background health loop
    /// automatically removes unhealthy workers and spawns replacements, so
    /// callers do not need to handle this directly.
    ///
    /// **Retryable** -- the pool replaces the dead worker transparently.
    #[error("worker health check failed: {0}")]
    HealthCheckFailed(String),

    /// The worker process exited while the server was waiting for a response.
    ///
    /// Typical causes: OOM kill, segfault in a native library (torch, stanza),
    /// or an unhandled Python exception that crashes the process. The exit
    /// code (if available) can distinguish signals from normal exits.
    ///
    /// **Retryable** -- the pool's health loop will detect the dead process
    /// and spawn a replacement. However, if the crash is deterministic (e.g.
    /// always triggered by a specific input), the replacement will crash too.
    #[error("{}", format_process_exited(*.code, stderr.as_deref()))]
    ProcessExited {
        /// Exit code of the worker process, if available.
        code: Option<i32>,
        /// Last lines of the worker's stderr, captured at crash time.
        ///
        /// Contains the Python traceback, OOM message, or other diagnostic
        /// output that explains WHY the worker died. `None` if stderr was
        /// empty or could not be read before the process fully exited.
        stderr: Option<String>,
    },

    /// The stdio JSON-lines protocol was violated: a request could not be
    /// serialized, a response could not be deserialized, or the response
    /// had the wrong `op` tag for the request that was sent.
    ///
    /// This points to a version mismatch between Rust and Python, or a bug
    /// in one side's serialization. Also used for IPC timeouts (e.g. a
    /// batch_infer that exceeds its per-item budget).
    ///
    /// **Terminal for this request** -- the worker's stdio stream may be
    /// desynchronized after a framing error. The pool should discard the
    /// worker (via `CheckedOutWorker::take()`) rather than returning it.
    #[error("worker protocol error: {0}")]
    Protocol(String),

    /// The worker understood the request but returned an application-level
    /// error (the `{"op":"error","error":"..."}` response) for a
    /// **runtime / per-request** failure.
    ///
    /// Examples: a transient model-inference failure on a malformed input,
    /// an external-API hiccup, a worker-side validation that depends on
    /// per-request payload. The error string comes directly from the
    /// Python side.
    ///
    /// **Retryable** -- a fresh worker may succeed on retry, especially
    /// when the underlying cause is per-request (different inputs, transient
    /// resource state).
    ///
    /// For deterministic bootstrap failures (missing model files, catalog
    /// download failures, unsupported language), the worker emits the
    /// `{"op":"error", "kind":"bootstrap"}` wire shape, which decodes into
    /// the [`Bootstrap`] variant instead — those are non-retryable.
    ///
    /// [`Bootstrap`]: WorkerError::Bootstrap
    #[error("worker returned error: {0}")]
    WorkerResponse(String),

    /// The worker reported a **bootstrap-class** failure: model loading,
    /// catalog download, package import, or any deterministic failure that
    /// would recur identically across worker restarts.
    ///
    /// Wire shape: `{"op":"error", "error":"<msg>", "kind":"bootstrap"}`.
    /// Emitted by the Python worker's request-loop exception handler when
    /// the underlying error is one of the typed bootstrap errors
    /// (`StanzaCatalogDownloadError`, `UnsupportedLanguageError`, missing
    /// HuggingFace model with offline mode on, etc.).
    ///
    /// **Terminal** -- retrying a deterministic bootstrap failure 3× with
    /// a full traceback per attempt is the bug shape this variant exists
    /// to prevent. The orchestrator classifies this as non-retryable and
    /// surfaces the typed message verbatim to the user (network error,
    /// disk full, authentication failure, etc. — all actionable).
    #[error("worker bootstrap error: {0}")]
    Bootstrap(String),

    /// Low-level I/O failure on the stdin/stdout pipes to the worker process.
    ///
    /// Usually means the pipe was closed (worker crashed) or a system-level
    /// I/O error occurred. Often accompanied or preceded by [`ProcessExited`].
    ///
    /// **Retryable** -- the pool spawns a replacement worker.
    ///
    /// [`ProcessExited`]: WorkerError::ProcessExited
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The worker could not start because the host-memory guard
    /// refused the per-spawn reservation (insufficient available RAM,
    /// or the host-coordinator's lease acquisition failed).
    ///
    /// Distinguishes RAM-pressure rejections from generic spawn
    /// failures so observability surfaces (`PoolMetrics`, doctor)
    /// can count host-memory pressure independently. The pool's
    /// `lifecycle::try_spawn_into_group` increments
    /// `memory_gate_rejections_total` on this variant.
    ///
    /// **Retryable** -- once host RAM is freed (other workers exit,
    /// idle timeouts fire), a fresh spawn can succeed.
    #[error("memory guard refused worker spawn: {0}")]
    MemoryGuard(#[from] crate::worker::memory_guard::MemoryGuardError),

    /// No worker could be found or spawned for the requested
    /// `(command, lang)` pair.
    ///
    /// Currently unused -- the pool lazily spawns workers on demand and
    /// returns [`SpawnFailed`] if spawning fails. This variant is reserved
    /// for future use (e.g. capability-gated dispatch where no worker
    /// advertises the required infer task).
    ///
    /// **Terminal** -- the server does not support this command/lang
    /// combination.
    ///
    /// [`SpawnFailed`]: WorkerError::SpawnFailed
    #[error("no worker available for command={command} lang={lang}")]
    NoWorker {
        /// Processing command that was requested.
        command: crate::api::ReleasedCommand,
        /// Language code that was requested.
        lang: crate::api::WorkerLanguage,
    },
}
