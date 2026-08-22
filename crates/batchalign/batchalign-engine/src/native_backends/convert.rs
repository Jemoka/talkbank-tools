//! `batchalign._core.backends.ConvertBackend` — configurable native wrapper.

use batchalign_core::{Backend, MediaFormat, PreparedAudio};
use batchalign_core::backends::ConvertBackend as CoreConvertBackend;
use pyo3::prelude::*;
use pyo3::types::PyCapsule;
use std::sync::Arc;

#[pyclass(name = "ConvertBackend", module = "batchalign._core.backends")]
#[derive(Clone)]
pub struct ConvertBackend {
    inner: Arc<CoreConvertBackend>,
}

#[pymethods]
impl ConvertBackend {
    #[new]
    fn py_new(format: &str) -> PyResult<Self> {
        let format = format.parse::<MediaFormat>().map_err(|message| {
            pyo3::exceptions::PyValueError::new_err(message)
        })?;
        Ok(Self {
            inner: Arc::new(CoreConvertBackend::new(format)),
        })
    }

    #[getter]
    fn __batchalign_native_arc__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let arc: Arc<dyn Backend> = self.inner.clone();
        let name = std::ffi::CString::new(super::NATIVE_ARC_CAPSULE_NAME)
            .expect("static capsule name");
        PyCapsule::new(py, arc, Some(name))
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }

    #[getter]
    fn batch_policy(&self) -> batchalign_core::BatchPolicy {
        self.inner.batch_policy()
    }

    #[getter]
    fn tasks(&self) -> Vec<batchalign_core::Task> {
        self.inner.tasks().to_vec()
    }

    /// Encode prepared interleaved float32 PCM without constructing a nested
    /// Pipeline. Cloud backends use this to obtain an in-memory media payload
    /// while sharing the Convert task's codec implementation.
    #[pyo3(signature = (pcm_f32le, sample_rate, channels, frame_count))]
    fn encode_prepared(
        &self,
        pcm_f32le: Vec<u8>,
        sample_rate: u32,
        channels: u16,
        frame_count: u64,
    ) -> PyResult<Vec<u8>> {
        self.inner
            .encode(&PreparedAudio {
                pcm_f32le,
                sample_rate,
                channels,
                frame_count,
            })
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("ConvertBackend(name={:?})", self.inner.name())
    }
}
