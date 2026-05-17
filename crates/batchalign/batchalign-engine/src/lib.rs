//! batchalign-engine — tokio runtime, per-backend batching, redb cache, and
//! the PyO3 `Pipeline` orchestrator (spec2.md §11–§13, §20–§21).
//!
//! Layered on top of `batchalign-core`: this crate owns no domain types,
//! only the runtime plumbing that drives `TaskRunner` + `Backend`.

#![allow(dead_code)]

pub mod backend_impl;
pub mod batcher;
pub mod cache;
pub mod engine;
pub mod metrics_writer;
pub mod native_backends;
pub mod pipeline;
pub mod progress_sink;
pub mod py_outcome;

#[cfg(feature = "extension-module")]
pub mod python;

pub use cache::{Cache, CachePolicy, CacheSpec, default_cache_path, nuke_cache};
pub use engine::{BatchalignEngine, EngineConfig};
pub use pipeline::Pipeline;

// Maturin builds this crate as the `batchalign._core` extension module.
// The `#[pymodule]` definition lives behind `extension-module` so plain
// `cargo build` (without the pyo3 host shim) can link the rlib half.
#[cfg(feature = "extension-module")]
#[pyo3::pymodule]
fn _core(py: pyo3::Python<'_>, m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    python::register(py, m)
}
