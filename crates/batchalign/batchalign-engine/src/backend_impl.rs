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

use std::ffi::CString;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use batchalign_core::{
    BAError, BAResult, Backend, BackendProgress, BatchPolicy, Task, TaskInput, TaskOutput,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyCapsule, PyList};

use crate::native_backends::NATIVE_ARC_CAPSULE_NAME;

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
    /// Constructs a `BackendImpl` from any Python-side backend instance.
    ///
    /// If the object is a Rust-native wrapper (any class in
    /// `batchalign._core.backends`, recognised via the
    /// `__batchalign_native_arc__` PyCapsule), this clones the inner
    /// `Arc<dyn Backend>` out of the capsule and returns
    /// `BackendImpl::Native`. The engine then drives the algorithm
    /// directly with no GIL acquisition and no JSON round-trip on the
    /// hot path. This is fully generic across native backends — adding
    /// a new one needs no edits here.
    ///
    /// Otherwise reads `obj.name`, `obj.batch_policy`, and dispatches to
    /// `batchalign.backends.base.declared_tasks(obj)` for the task set
    /// (spec §10.1) and stores the handle as `BackendImpl::Python`.
    pub fn from_py(obj: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| {
            let bound = obj.bind(py);

            if let Ok(attr) = bound.getattr("__batchalign_native_arc__") {
                if let Ok(cap) = attr.cast::<PyCapsule>() {
                    let expected =
                        CString::new(NATIVE_ARC_CAPSULE_NAME).expect("static capsule name");
                    if let Ok(ptr) = cap.pointer_checked(Some(&expected)) {
                        // SAFETY: every native wrapper macro publishes a
                        // capsule with this exact name and an
                        // `Arc<dyn Backend>` payload (see `native_backend!`).
                        // `pointer_checked` validated the name, so the
                        // payload type is what we expect.
                        let arc_ref: &Arc<dyn Backend> =
                            unsafe { &*(ptr.as_ptr() as *const Arc<dyn Backend>) };
                        return Ok(BackendImpl::Native(arc_ref.clone()));
                    }
                }
            }

            let name: String = bound.getattr("name")?.extract().map_err(|e| {
                pyo3::exceptions::PyTypeError::new_err(format!("backend.name must be str: {e}"))
            })?;

            // batch_policy is either a BatchPolicy #[pyclass] or a duck-typed
            // object with max_size + window_ms attributes. We extract via the
            // typed path first.
            let policy_obj = bound.getattr("batch_policy")?;
            let batch_policy: BatchPolicy = match policy_obj.extract::<BatchPolicy>() {
                Ok(p) => p,
                Err(_) => {
                    let max_size: usize = policy_obj.getattr("max_size")?.extract()?;
                    let window_ms: u64 = policy_obj.getattr("window_ms")?.extract()?;
                    BatchPolicy {
                        max_size,
                        window_ms,
                    }
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
        // Always go through the progress-threading entry point — the
        // batcher uses `call_with_progress`, this `call` exists only
        // for legacy callers (tests) that don't want a progress channel.
        Backend::call_with_progress(self, batch, Arc::new(batchalign_core::NullBackendProgress))
    }

    fn call_with_progress(
        &self,
        batch: Vec<TaskInput>,
        progress: Arc<dyn BackendProgress>,
    ) -> BAResult<Vec<TaskOutput>> {
        match self {
            BackendImpl::Native(b) => b.call_with_progress(batch, progress),
            BackendImpl::Python(h) => call_py_backend(h, batch, progress),
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
///
/// The `progress` channel is exposed to Python as a `progress(completed,
/// total)` keyword-only callable. The pyclass below holds an `Arc` clone
/// of the trait object so its lifetime is owned, not borrowed; Python
/// can hold the callable for the duration of `call` without lifetime
/// gymnastics. Backends MUST drop their reference before returning
/// (don't stash it across calls).
fn call_py_backend(
    h: &PyBackendHandle,
    batch: Vec<TaskInput>,
    progress: Arc<dyn BackendProgress>,
) -> BAResult<Vec<TaskOutput>> {
    let request_json = serde_json::to_string(&batch)
        .map_err(|e| BAError::Worker(format!("serialize TaskInput batch: {e}")))?;

    Python::attach(|py| -> BAResult<Vec<TaskOutput>> {
        let json_mod = py
            .import("json")
            .map_err(|e| BAError::Worker(format!("import json: {e}")))?;
        let proto_mod = py
            .import("batchalign._core.proto")
            .map_err(|e| BAError::Worker(format!("import batchalign._core.proto: {e}")))?;

        // Rehydrate: tagged-dict JSON -> typed proto dataclass instances.
        // Backends do `isinstance(item, MorphosyntaxInput)` checks, so we
        // can't hand them plain dicts.
        let tagged_dicts = json_mod
            .getattr("loads")
            .and_then(|f| f.call1((request_json,)))
            .map_err(|e| BAError::Worker(format!("json.loads on serialized batch: {e}")))?;
        let py_batch = proto_mod
            .getattr("rebuild_tagged_inputs")
            .and_then(|f| f.call1((tagged_dicts,)))
            .map_err(|e| BAError::Worker(format!("rebuild_tagged_inputs: {e}")))?;

        // Build a Python callable that forwards `progress(completed,
        // total)` back to the Rust `BackendProgress` via the pyclass
        // defined below. We clone the Arc so the pyclass owns its own
        // reference; backends must not stash this callable past their
        // `call` return.
        let progress_cb = Py::new(
            py,
            PyProgressCallback {
                progress: progress.clone(),
            },
        )
        .map_err(|e| BAError::Worker(format!("build progress callable: {e}")))?
        .into_any();
        // Backends accept `progress` as a keyword argument; passing it
        // positional would break the legacy `call(self, batch)` shape.
        // Backends written before this change ignore unknown kwargs via
        // `**kwargs` in the base ABC.
        let kwargs = pyo3::types::PyDict::new(py);
        kwargs
            .set_item("progress", progress_cb)
            .map_err(|e| BAError::Worker(format!("set progress kwarg: {e}")))?;

        let result = h
            .obj
            .bind(py)
            .call_method("call", (py_batch,), Some(&kwargs))
            .map_err(|e| {
                if std::env::var_os("BATCHALIGN_CLI_VERBOSE_TRACEBACKS").is_some() {
                    BAError::Worker(format_py_exception(py, "Backend.call raised", e))
                } else {
                    BAError::Worker(format!("Backend.call raised: {e}"))
                }
            })?;

        // Reverse the rehydration: typed *Output dataclasses -> tagged-dict
        // JSON -> Vec<TaskOutput>. Pass the original input tags through
        // so UTR (which shares `AsrOutput` as its Python wire type) gets
        // serialized back with the `"Utr"` discriminator rather than
        // being misclassified as `"Asr"`.
        let input_tags: Vec<&'static str> = batch
            .iter()
            .map(|t| match t {
                TaskInput::Asr(_) => "Asr",
                TaskInput::Fa(_) => "Fa",
                TaskInput::Speaker(_) => "Speaker",
                TaskInput::UtSeg(_) => "UtSeg",
                TaskInput::Utr(_) => "Utr",
                TaskInput::Morphosyntax(_) => "Morphosyntax",
                TaskInput::Translate(_) => "Translate",
                TaskInput::Coref(_) => "Coref",
                TaskInput::Compare(_) => "Compare",
            })
            .collect();
        let response_tagged = proto_mod
            .getattr("serialize_tagged_outputs")
            .and_then(|f| f.call1((result, input_tags)))
            .map_err(|e| BAError::Worker(format!("serialize_tagged_outputs: {e}")))?;
        let response_str = json_mod
            .getattr("dumps")
            .and_then(|f| f.call1((response_tagged,)))
            .and_then(|s| s.extract::<String>())
            .map_err(|e| BAError::Worker(format!("json.dumps on backend response: {e}")))?;

        let outputs: Vec<TaskOutput> = serde_json::from_str(&response_str)
            .map_err(|e| BAError::Worker(format!("deserialize Vec<TaskOutput>: {e}")))?;
        Ok(outputs)
    })
}

fn format_py_exception(py: Python<'_>, context: &str, err: PyErr) -> String {
    let fallback = err.to_string();
    let exc = err.into_value(py);
    let traceback = py
        .import("traceback")
        .and_then(|m| m.getattr("format_exception"))
        .and_then(|f| f.call1((exc.bind(py),)))
        .and_then(|parts| parts.extract::<Vec<String>>())
        .map(|parts| parts.concat())
        .unwrap_or_else(|_| fallback.clone());

    let traceback = traceback.trim_end();
    if traceback.is_empty() {
        format!("{context}: {fallback}")
    } else {
        format!("{context}: {fallback}\n{traceback}")
    }
}

/// Python-side callable that forwards `progress(completed, total)`
/// invocations back to a Rust [`BackendProgress`] held inside the Arc.
///
/// The pyclass owns its `Arc` clone, so its lifetime is decoupled from
/// the `call_py_backend` stack frame; if a backend (incorrectly) stashes
/// the callable beyond the call, the Arc keeps the underlying
/// `BackendProgress` alive — at worst the late ticks emit into a sink
/// that may have moved on. No UB, just noise. Backends SHOULD drop
/// their callable reference before returning.
#[pyclass]
struct PyProgressCallback {
    progress: Arc<dyn BackendProgress>,
}

#[pymethods]
impl PyProgressCallback {
    /// `progress(completed, total)` from Python → `BackendProgress::tick`.
    fn __call__(&self, completed: u64, total: u64) {
        self.progress.tick(completed, total);
    }
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
