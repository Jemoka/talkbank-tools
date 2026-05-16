//! `PyOutcome` — Python-facing wrapper around `BAValue`.
//!
//! `BAValue` is a plain Rust enum (no `#[pyclass]`) because several of its
//! variants carry types (`Chat<Validated>` wrapping `talkbank_model::ChatFile`,
//! `MetricsArtifact` with a `serde_json::Value` map) that cannot trivially be
//! exposed as pyclasses. The Python surface only needs to know: which source
//! produced the outcome, whether it failed, and how to write the result. This
//! wrapper exposes exactly that.

use std::path::PathBuf;
use std::sync::Mutex;

use batchalign_core::{BAError, BAValue, SourceId};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Opaque PyO3 handle around a single pipeline outcome.
///
/// The wrapped `BAValue` is held in a `Mutex<Option<...>>` so `write()` can
/// take it by value (it serializes the CHAT or metrics to disk). After
/// `write()`, the cell is empty — calling `write()` twice raises.
#[pyclass(name = "Outcome", module = "batchalign._core")]
pub struct PyOutcome {
    inner: Mutex<Option<BAValue>>,
    source_id: SourceId,
    kind_str: String,
    failed: bool,
}

impl PyOutcome {
    pub fn from_value(value: BAValue) -> Self {
        let source_id = value.source_id();
        let kind_str = value.kind().to_string();
        let failed = value.is_failed();
        PyOutcome {
            inner: Mutex::new(Some(value)),
            source_id,
            kind_str,
            failed,
        }
    }
}

#[pymethods]
impl PyOutcome {
    /// Identifier of the source that produced this outcome.
    #[getter]
    fn source_id(&self) -> String {
        self.source_id.as_str().to_owned()
    }

    /// `"media" | "chat" | "paired" | "metrics" | "failed"`.
    #[getter]
    fn kind(&self) -> String {
        self.kind_str.clone()
    }

    /// `True` if this outcome corresponds to a per-value failure.
    #[getter]
    fn is_failed(&self) -> bool {
        self.failed
    }

    /// Human-readable error string when `is_failed()` is true; otherwise `""`.
    #[getter]
    fn error(&self) -> String {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return String::new(),
        };
        match guard.as_ref() {
            Some(BAValue::Failed { error, .. }) => format!("{error}"),
            _ => String::new(),
        }
    }

    /// Write the outcome to disk. Consumes the outcome (calling twice raises).
    fn write(&self, path: PathBuf) -> PyResult<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| PyRuntimeError::new_err("outcome lock poisoned"))?;
        let value = guard
            .take()
            .ok_or_else(|| PyValueError::new_err("outcome already written"))?;
        match value.write(&path) {
            Ok(()) => Ok(()),
            Err(BAError::Io(e)) => Err(PyRuntimeError::new_err(format!("write {}: {e}", path.display()))),
            Err(other) => Err(PyRuntimeError::new_err(format!("write {}: {other}", path.display()))),
        }
    }
}
