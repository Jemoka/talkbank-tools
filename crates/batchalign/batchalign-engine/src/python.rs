//! `#[pymodule]` registration for the `batchalign._core` extension.
//!
//! Two layers attach types/functions to the Python module:
//!  1. `batchalign_core::python::register` — Task, BAValue, MediaInput,
//!     BatchPolicy, ProgressEvent, ProgressKind, SourceId, etc.
//!  2. This module's `register` — Pipeline, CacheSpec, CachePolicy, and the
//!     `nuke_cache` free function.

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::cache::{default_cache_path_py, nuke_cache, CachePolicy, CacheSpec};
use crate::dp_py::dp_align;
use crate::native_backends;
use crate::pipeline::Pipeline;

/// Attach this crate's types + the core crate's types to `m`.
///
/// Called from the `#[pymodule]` shim in `lib.rs`.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Install a tracing subscriber driven by `RUST_LOG`/`BATCHALIGN_LOG`
    // so cache + runner instrumentation is visible from the Python CLI.
    // try_init is a no-op if a subscriber is already installed.
    let filter = tracing_subscriber::EnvFilter::try_from_env("BATCHALIGN_LOG")
        .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .without_time()
        .try_init();

    // Engine-side types.
    m.add_class::<Pipeline>()?;
    m.add_class::<CacheSpec>()?;
    m.add_class::<CachePolicy>()?;
    // Native backends live under `batchalign._core.backends` — see
    // `native_backends/mod.rs` for the macro-generated wrappers.
    native_backends::register(py, m)?;
    m.add_function(wrap_pyfunction!(nuke_cache, m)?)?;
    m.add_function(wrap_pyfunction!(default_cache_path_py, m)?)?;
    m.add_function(wrap_pyfunction!(dp_align, m)?)?;
    // VERGEN_GIT_SHA baked at compile time (build.rs). Surfaces to
    // `batchalign3 version` and the X-Batchalign-SHA response header.
    // option_env! handles the Bazel path (where build.rs's
    // cargo:rustc-env directive doesn't propagate through rules_rust).
    m.add(
        "BATCHALIGN_GIT_SHA",
        option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
    )?;

    // Core types (Task, BAValue, MediaInput, BatchPolicy, ProgressEvent,
    // ProgressKind, SourceId, ...). The core crate owns the registration so
    // engine doesn't need to know which types exist.
    batchalign_core::python::register(py, m)?;
    Ok(())
}
