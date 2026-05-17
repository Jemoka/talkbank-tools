//! Pyo3 wrappers for Rust-native `Backend` implementations.
//!
//! A Python user constructs e.g. `ba.CompareBackend()` and hands it to
//! `Pipeline(backends=[...])`. The engine recognises these wrappers in
//! `BackendImpl::from_py` and routes them as `BackendImpl::Native(Arc<dyn
//! Backend>)`, bypassing the JSON / GIL round-trip used for Python-side
//! backends. That's how the engine gets GIL-free in-process parallelism for
//! pure-Rust tasks like Compare.

use std::sync::Arc;

use batchalign_core::backends::CompareBackend as CoreCompareBackend;
use batchalign_core::{BatchPolicy as CoreBatchPolicy, Task as CoreTask};
use batchalign_core::Backend;
use pyo3::prelude::*;

/// Pyo3 wrapper around the native `batchalign_core::backends::CompareBackend`.
///
/// Exposes a Python-callable surface that *also* duck-types as a Python
/// backend (`name`, `batch_policy`, `tasks` properties + a `call(batch)`
/// fallback) so the engine treats it uniformly. In practice the engine
/// short-circuits on the `__batchalign_native__` marker below and never
/// invokes the Python-side `call()`.
#[pyclass(name = "CompareBackend", module = "batchalign._core")]
#[derive(Clone)]
pub struct PyCompareBackend {
    inner: Arc<CoreCompareBackend>,
}

impl PyCompareBackend {
    /// Borrow the inner `Arc<dyn Backend>` so `BackendImpl::from_py` can use
    /// it directly without going through Python.
    pub fn as_backend(&self) -> Arc<dyn Backend> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyCompareBackend {
    #[new]
    fn py_new() -> Self {
        Self {
            inner: Arc::new(CoreCompareBackend::new()),
        }
    }

    /// Marker attribute the engine sniffs for during backend registration —
    /// presence means "unwrap me into a native `Arc<dyn Backend>`".
    #[getter]
    fn __batchalign_native__(&self) -> bool {
        true
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }

    #[getter]
    fn batch_policy(&self) -> CoreBatchPolicy {
        self.inner.batch_policy()
    }

    /// Tasks declared by this backend. Returned as a list of `Task` enum
    /// values so `batchalign.backends.base.declared_tasks` can read it for
    /// metadata-only inspection (the engine never goes through this path
    /// for native backends — the marker short-circuits it).
    #[getter]
    fn tasks(&self) -> Vec<CoreTask> {
        self.inner.tasks().to_vec()
    }

    fn __repr__(&self) -> String {
        format!("CompareBackend(name={:?})", self.inner.name())
    }
}
