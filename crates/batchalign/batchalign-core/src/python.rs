//! PyO3 bindings for the core types.
//!
//! The engine crate's `#[pymodule]` calls [`register`] to attach these
//! classes onto its compiled `batchalign._core` module. Core never builds
//! its own `cdylib`.

use crate::backends::BatchPolicy;
use crate::utils::MediaInput;
use crate::base::{ProgressEvent, ProgressKind};
use crate::base::Task;
use crate::utils::SourceId;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Attach core's PyO3 classes onto `m`. The engine crate calls this from
/// its `#[pymodule]` registration function.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Task>()?;
    m.add_class::<BatchPolicy>()?;
    m.add_class::<MediaInput>()?;
    m.add_class::<ProgressEvent>()?;
    m.add_class::<ProgressKind>()?;
    m.add_class::<SourceId>()?;
    Ok(())
}

#[pymethods]
impl BatchPolicy {
    #[new]
    #[pyo3(signature = (max_size=32, window_ms=50))]
    fn py_new(max_size: usize, window_ms: u64) -> Self {
        Self {
            max_size,
            window_ms,
        }
    }

    #[staticmethod]
    #[pyo3(name = "one")]
    fn py_one() -> Self {
        BatchPolicy::one()
    }

    #[staticmethod]
    #[pyo3(name = "fixed")]
    fn py_fixed(n: usize) -> Self {
        BatchPolicy::fixed(n)
    }
}

#[pymethods]
impl SourceId {
    #[new]
    fn py_new(s: &str) -> PyResult<Self> {
        SourceId::try_new(s).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn __str__(&self) -> String {
        self.as_str().to_owned()
    }

    fn __repr__(&self) -> String {
        format!("SourceId({:?})", self.as_str())
    }
}

#[pymethods]
impl MediaInput {
    #[new]
    #[pyo3(signature = (path, source_id))]
    fn py_new(path: std::path::PathBuf, source_id: SourceId) -> Self {
        Self::new(source_id, path)
    }
}
