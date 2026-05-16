//! `BackendImpl` (spec2.md §10.3): the kernel-side wrapper that lets the
//! engine treat native Rust backends and Python-implemented backends
//! uniformly.
//!
//! The native side just delegates to the `batchalign_core::Backend` trait.
//! The Python side caches `name`, `tasks`, and `batch_policy` at
//! construction so we don't re-acquire the GIL for metadata reads, and
//! routes `call()` through Python::attach + json round-trip.
//!
//! Why a JSON round-trip rather than `pyo3-serde`/`pythonize`? Neither
//! crate is in the workspace lockfile yet. The proto types serialize
//! cleanly to JSON; Python sees lists of dicts that `_core/proto.py`
//! can shape into typed dataclasses. When schema codegen lands (spec
//! §22.7), we'll revisit the conversion layer.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use batchalign_core::{BAError, BAResult, Backend, BatchPolicy, Task, TaskInput, TaskOutput};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList};

/// Either a Rust-native backend behind an `Arc<dyn Backend>` or a Python
/// backend held as a `Py<PyAny>` plus pre-introspected metadata.
pub enum BackendImpl {
    Native(Arc<dyn Backend>),
    Python(PyBackendHandle),
}

/// Python backend handle with metadata cached at construction time.
///
/// `obj` is a `Py<PyAny>` (GIL-independent reference). The metadata fields
/// are read once via `Python::attach` in `from_py`; subsequent reads from
/// the engine never need the GIL.
pub struct PyBackendHandle {
    obj: Py<PyAny>,
    name: String,
    tasks: Vec<Task>,
    batch_policy: BatchPolicy,
}

impl BackendImpl {
    /// Constructs a `BackendImpl::Python` from a Python `Backend` instance.
    ///
    /// Reads `obj.name` (property), `obj.batch_policy` (property), and
    /// dispatches to `batchalign.backends.base.declared_tasks(obj)` for the
    /// task set (spec §10.1).
    pub fn from_py(obj: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| {
            let bound = obj.bind(py);

            let name: String = bound
                .getattr("name")?
                .extract()
                .map_err(|e| pyo3::exceptions::PyTypeError::new_err(format!("backend.name must be str: {e}")))?;

            // batch_policy is either a BatchPolicy #[pyclass] or a duck-typed
            // object with max_size + window_ms attributes. We extract via the
            // typed path first.
            let policy_obj = bound.getattr("batch_policy")?;
            let batch_policy: BatchPolicy = match policy_obj.extract::<BatchPolicy>() {
                Ok(p) => p,
                Err(_) => {
                    let max_size: usize = policy_obj.getattr("max_size")?.extract()?;
                    let window_ms: u64 = policy_obj.getattr("window_ms")?.extract()?;
                    BatchPolicy { max_size, window_ms }
                }
            };

            let helper = py
                .import("batchalign.backends.base")
                .and_then(|m| m.getattr("declared_tasks"))
                .map_err(|e| {
                    pyo3::exceptions::PyImportError::new_err(format!(
                        "batchalign.backends.base.declared_tasks unavailable: {e}"
                    ))
                })?;
            let tasks_obj = helper.call1((bound,))?;
            let tasks_list = tasks_obj.downcast::<PyList>()?;
            let mut tasks: Vec<Task> = Vec::with_capacity(tasks_list.len());
            for t in tasks_list.iter() {
                tasks.push(t.extract::<Task>()?);
            }

            Ok(BackendImpl::Python(PyBackendHandle {
                obj,
                name,
                tasks,
                batch_policy,
            }))
        })
    }

    /// Constructs a `BackendImpl::Native` from any `Arc<dyn Backend>`.
    pub fn native(b: Arc<dyn Backend>) -> Self {
        BackendImpl::Native(b)
    }
}

impl Backend for BackendImpl {
    fn name(&self) -> &str {
        match self {
            BackendImpl::Native(b) => b.name(),
            BackendImpl::Python(h) => &h.name,
        }
    }

    fn tasks(&self) -> &[Task] {
        match self {
            BackendImpl::Native(b) => b.tasks(),
            BackendImpl::Python(h) => &h.tasks,
        }
    }

    fn batch_policy(&self) -> BatchPolicy {
        match self {
            BackendImpl::Native(b) => b.batch_policy(),
            BackendImpl::Python(h) => h.batch_policy,
        }
    }

    fn call(&self, batch: Vec<TaskInput>) -> BAResult<Vec<TaskOutput>> {
        match self {
            BackendImpl::Native(b) => b.call(batch),
            BackendImpl::Python(h) => call_py_backend(h, batch),
        }
    }
}

/// Calls a Python backend by:
/// 1. Serializing the Vec<TaskInput> to JSON (one array, tagged variants).
/// 2. Acquiring the GIL.
/// 3. Letting Python `json.loads` reshape it into a list of dicts the
///    backend understands (per the `_core/proto.py` shouty-mirrored types).
/// 4. Calling `obj.call(py_batch)`.
/// 5. Re-serializing the returned list (via `json.dumps`) and parsing back
///    to `Vec<TaskOutput>` on the Rust side.
///
/// The round-trip through JSON is the simplest erasure that doesn't pull in
/// a serde<->python adapter dep. Cost: two json passes per batch dispatch;
/// dwarfed by the inference cost the call is gating.
fn call_py_backend(h: &PyBackendHandle, batch: Vec<TaskInput>) -> BAResult<Vec<TaskOutput>> {
    let request_json = serde_json::to_string(&batch)
        .map_err(|e| BAError::Worker(format!("serialize TaskInput batch: {e}")))?;

    Python::attach(|py| -> BAResult<Vec<TaskOutput>> {
        let json_mod = py
            .import("json")
            .map_err(|e| BAError::Worker(format!("import json: {e}")))?;
        let py_batch = json_mod
            .getattr("loads")
            .and_then(|f| f.call1((request_json,)))
            .map_err(|e| BAError::Worker(format!("json.loads on serialized batch: {e}")))?;

        let result = h
            .obj
            .bind(py)
            .call_method1("call", (py_batch,))
            .map_err(|e| BAError::Worker(format!("Backend.call raised: {e}")))?;

        let response_str = json_mod
            .getattr("dumps")
            .and_then(|f| f.call1((result,)))
            .and_then(|s| s.extract::<String>())
            .map_err(|e| BAError::Worker(format!("json.dumps on backend response: {e}")))?;

        let outputs: Vec<TaskOutput> = serde_json::from_str(&response_str)
            .map_err(|e| BAError::Worker(format!("deserialize Vec<TaskOutput>: {e}")))?;
        Ok(outputs)
    })
}

/// Returned by `Pipeline::py_new` when a non-Backend object is passed.
pub fn ensure_is_backend(_obj: &Bound<'_, PyAny>) -> Result<()> {
    // We don't strictly enforce the marker-ABC subclass here — the helper
    // `declared_tasks` already errors if no marker ABC matched. Leaving a
    // hook so a future, stricter check can land without API churn.
    if false {
        bail!("placeholder");
    }
    Ok(())
}

/// Helper for tests: trivial native backend that returns a canned response.
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers {
    use super::*;

    pub struct FixedBackend {
        pub name: String,
        pub tasks: Vec<Task>,
        pub policy: BatchPolicy,
        pub responder: Arc<dyn Fn(TaskInput) -> BAResult<TaskOutput> + Send + Sync>,
    }

    impl Backend for FixedBackend {
        fn name(&self) -> &str {
            &self.name
        }
        fn tasks(&self) -> &[Task] {
            &self.tasks
        }
        fn batch_policy(&self) -> BatchPolicy {
            self.policy
        }
        fn call(&self, batch: Vec<TaskInput>) -> BAResult<Vec<TaskOutput>> {
            batch.into_iter().map(|i| (self.responder)(i)).collect()
        }
    }

    /// Convenience: wrap a closure into an Arc<dyn Backend>.
    pub fn fixed(
        name: &str,
        tasks: Vec<Task>,
        policy: BatchPolicy,
        responder: impl Fn(TaskInput) -> BAResult<TaskOutput> + Send + Sync + 'static,
    ) -> Arc<dyn Backend> {
        Arc::new(FixedBackend {
            name: name.to_string(),
            tasks,
            policy,
            responder: Arc::new(responder),
        })
    }

    // Silence "unused" warnings on the optional helper path.
    pub fn _touch() -> anyhow::Result<()> {
        Err(anyhow!("test_helpers is for tests only"))
    }
}
