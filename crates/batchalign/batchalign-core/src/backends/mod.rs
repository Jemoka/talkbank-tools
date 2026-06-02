//! The `Backend` trait — inference adapter.
//!
//! A backend declares which tasks it services (`tasks()`), its preferred
//! batching policy (`batch_policy()`), and a single `call()` that takes a
//! batch of `TaskInput` (potentially mixed across the backend's declared
//! tasks) and returns same-length, index-aligned `TaskOutput`s.
//!
//! Atomic-call providers (e.g. Rev.AI: one submission yields both ASR and
//! Speaker outputs) dedupe by `source_id` inside `call`.
//!
//! See `spec2.md` §10.

use crate::base::Task;
use crate::base::{BackendProgress, NullBackendProgress, TaskInput, TaskOutput};
use crate::utils::BAResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod compare;
pub use compare::CompareBackend;

/// Backend-call failure.
#[derive(Debug, Error)]
pub enum BackendCallError {
    /// The backend rejected an input variant it doesn't service.
    #[error("backend does not handle variant: {0}")]
    UnsupportedVariant(String),
    /// The backend returned an output of the wrong length / variant.
    #[error("backend returned wrong-shape batch: {0}")]
    ShapeMismatch(String),
    /// Anything else — networking, model load, GPU OOM …
    #[error("backend error: {0}")]
    Other(String),
}

/// Metadata read once per backend at registration. Stored alongside the
/// `Arc<dyn Backend>` so the engine doesn't acquire the GIL just to read
/// constants. Mirrors the shape of `Backend::name() / tasks() / batch_policy()`.
#[derive(Clone, Debug)]
pub struct BackendMeta {
    /// Stable backend identity (used as the cache namespace key).
    pub name: String,
    /// Tasks this backend services.
    pub tasks: Vec<Task>,
    /// Preferred batching window/size.
    pub batch_policy: BatchPolicy,
}

/// Inference adapter. Implementations live in batchalign-engine
/// (native rust) or python/batchalign/backends (Python via PyO3 wrapper).
pub trait Backend: Send + Sync {
    /// Stable name; used as the cache namespace key.
    fn name(&self) -> &str;
    /// Tasks this backend handles. The batcher routes `dispatch` calls for
    /// any of these tasks into this backend's queue.
    fn tasks(&self) -> &[Task];
    /// Preferred batching policy.
    fn batch_policy(&self) -> BatchPolicy;
    /// Process a same-batch slice; output length must equal input length and
    /// index alignment must be preserved.
    ///
    /// Backends that want to report intra-call progress should override
    /// [`call_with_progress`] instead and leave this delegating shim alone.
    fn call(&self, batch: Vec<TaskInput>) -> BAResult<Vec<TaskOutput>> {
        // Legacy entry: synthesizes a no-op progress channel and routes
        // through the new entry point. Both default impls forward to
        // each other; concrete backends MUST override at least one of
        // them (otherwise infinite recursion — caught at first dispatch).
        self.call_with_progress(batch, std::sync::Arc::new(NullBackendProgress))
    }

    /// Process a batch and let the backend optionally report intra-call
    /// progress via `progress.tick(completed, total)`.
    ///
    /// The handle is owned (an `Arc`) so the implementation can clone it
    /// into a Python callable or any other longer-lived bridge without
    /// fighting the borrow checker.
    ///
    /// Backends with no meaningful sub-progress can ignore `progress`
    /// (the default forwards to `call` and discards the handle).
    ///
    /// The composition contract with [`crate::base::ScaledProgress`] is:
    /// the runner has already advanced the outer step via `start_step()`
    /// before issuing this call; the backend may freely tick within
    /// `[0, total]` and the wrapper rescales those into the outer bar.
    fn call_with_progress(
        &self,
        batch: Vec<TaskInput>,
        _progress: std::sync::Arc<dyn BackendProgress>,
    ) -> BAResult<Vec<TaskOutput>> {
        self.call(batch)
    }
}

/// Batching policy. Engine-side batcher loops use these to bound how long it
/// waits for more inputs before flushing a partial batch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all, set_all))]
pub struct BatchPolicy {
    /// Maximum batch size before forced flush.
    pub max_size: usize,
    /// Maximum wait time in milliseconds before flushing a partial batch.
    pub window_ms: u64,
}

impl BatchPolicy {
    /// Atomic-call: never batch.
    pub const fn one() -> Self {
        Self {
            max_size: 1,
            window_ms: 0,
        }
    }

    /// Fixed-size batches, mid-latency window.
    pub const fn fixed(n: usize) -> Self {
        Self {
            max_size: n,
            window_ms: 50,
        }
    }
}

impl Default for BatchPolicy {
    fn default() -> Self {
        Self {
            max_size: 32,
            window_ms: 50,
        }
    }
}
