//! `#[pymodule]` registration for the `batchalign._core` extension.
//!
//! Two layers attach types/functions to the Python module:
//!  1. `batchalign_core::python::register` — Task, BAValue, MediaInput,
//!     BatchPolicy, ProgressEvent, ProgressKind, SourceId, etc.
//!  2. This module's `register` — Pipeline, CacheSpec, CachePolicy, and the
//!     `nuke_cache` free function.

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::cache::{nuke_cache, CachePolicy, CacheSpec};
use crate::pipeline::Pipeline;

/// Attach this crate's types + the core crate's types to `m`.
///
/// Called from the `#[pymodule]` shim in `lib.rs`.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Engine-side types.
    m.add_class::<Pipeline>()?;
    m.add_class::<CacheSpec>()?;
    m.add_class::<CachePolicy>()?;
    m.add_function(wrap_pyfunction!(nuke_cache, m)?)?;

    // Core types (Task, BAValue, MediaInput, BatchPolicy, ProgressEvent,
    // ProgressKind, SourceId, ...). The core crate owns the registration so
    // engine doesn't need to know which types exist.
    batchalign_core::python::register(py, m)?;
    Ok(())
}
