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

use crate::utils::BAResult;
use crate::base::Task;
use crate::base::{TaskInput, TaskOutput};
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
    fn call(&self, batch: Vec<TaskInput>) -> BAResult<Vec<TaskOutput>>;
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
