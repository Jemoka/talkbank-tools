//! Python worker process manager: spawn, health-check, dispatch.
//!
//! This crate manages Python worker child processes that do the actual ML
//! inference. The Rust binary is the control plane; Python workers are the
//! data plane.
//!
//! # Architecture
//!
//! ```text
//! WorkerPool
//!   ├── WorkerHandle("infer:morphosyntax", "eng") → morphosyntax model host
//!   ├── WorkerHandle("infer:fa", "eng")           → forced-alignment model host
//!   └── WorkerHandle("infer:asr", "eng")          → ASR model host
//! ```
//!
//! Workers are spawned lazily on first request, health-checked periodically,
//! restarted on failure, and idle-timed out after inactivity.

pub mod artifacts_v2;
pub mod asr_request_v2;
pub mod asr_result_v2;
pub mod avqi_request_v2;
pub mod error;
pub mod fa_result_v2;
pub mod handle;
pub mod memory_guard;
pub mod opensmile_request_v2;
pub mod pool;
pub(crate) mod provider_credentials;
pub mod python;
pub mod registry;
pub mod request_builder_v2;
pub mod speaker_request_v2;
pub mod speaker_result_v2;
pub(crate) mod target;
pub mod tcp_handle;
pub mod text_request_v2;
pub mod text_result_v2;

// Re-export wire-format types from types::worker so that
// `crate::worker::InferTask` etc. continues to resolve.
pub use crate::types::worker::*;
pub use target::{WorkerBootstrapMode, WorkerProfile, WorkerTarget};

// ---------------------------------------------------------------------------
// ensure_task IPC types (shared between sequential and concurrent paths)
// ---------------------------------------------------------------------------

/// Status of an `ensure_task` model-loading IPC call.
///
/// The Python worker returns `loaded` or `already_loaded`. The Rust-side
/// cache adds `already_loaded_cached` when it skips IPC entirely.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsureTaskStatus {
    /// Models were loaded on demand (first call for this task).
    Loaded,
    /// Python confirmed the task was already loaded (IPC round-trip happened).
    AlreadyLoaded,
    /// Rust-side cache confirmed the task was loaded (no IPC round-trip).
    AlreadyLoadedCached,
}

impl std::fmt::Display for EnsureTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loaded => f.write_str("loaded"),
            Self::AlreadyLoaded => f.write_str("already_loaded"),
            Self::AlreadyLoadedCached => f.write_str("already_loaded_cached"),
        }
    }
}

/// Response from the `ensure_task` IPC operation.
///
/// Used by both the sequential worker path (`WorkerHandle::ensure_task`)
/// and the concurrent GPU path (`SharedGpuWorker::ensure_task`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EnsureTaskResponse {
    /// Whether the task was already loaded or had to be switched.
    pub status: EnsureTaskStatus,
    /// The active task name after the ensure operation completes.
    pub task: String,
    /// Wall-clock seconds the worker spent loading or switching the task.
    pub elapsed_s: f64,
}
