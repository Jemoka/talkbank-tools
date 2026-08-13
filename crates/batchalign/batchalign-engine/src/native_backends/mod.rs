//! Auto-wrapped pyo3 surface for Rust-native `Backend` implementations.
//!
//! Every native backend lives in its own submodule here and is generated
//! from a single core type by the [`native_backend!`] macro: the macro
//! emits a `#[pyclass]` that wraps `Arc<CoreBackend>`, exposes the
//! `Backend` trait getters (`name`, `tasks`, `batch_policy`), and stashes
//! the `Arc<dyn Backend>` in a `PyCapsule` under
//! `__batchalign_native_arc__`. `BackendImpl::from_py` reads that capsule
//! and routes the backend as `BackendImpl::Native` — no JSON round-trip,
//! no per-backend code in the engine.
//!
//! The wrappers are attached to a Python submodule at
//! `batchalign._core.backends`, registered via [`register`]. To add a new
//! native backend: implement it in `batchalign-core/src/backends/`, add a
//! `pub mod foo;` file here that invokes `native_backend!`, and add an
//! `m.add_class::<foo::Foo>()?` line to [`register`].

use pyo3::prelude::*;
use pyo3::types::PyModule;

pub mod compare;
pub mod convert;

/// Build the `batchalign._core.backends` submodule, populate it with all
/// native backend wrappers, and attach it to the parent `_core` module.
///
/// Also publishes the submodule in `sys.modules` so
/// `from batchalign._core.backends import …` works (pyo3 submodules are
/// not otherwise visible to `import`).
pub fn register<'py>(py: Python<'py>, parent: &Bound<'py, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "backends")?;
    m.add_class::<compare::CompareBackend>()?;
    m.add_class::<convert::ConvertBackend>()?;

    parent.add_submodule(&m)?;
    let sys = py.import("sys")?;
    sys.getattr("modules")?
        .set_item("batchalign._core.backends", &m)?;
    Ok(())
}

/// Capsule name embedded in every native-backend wrapper. Engine-side
/// downcast uses this to validate the capsule before reading the Arc.
pub const NATIVE_ARC_CAPSULE_NAME: &str = "batchalign_native_backend";

/// Generate a `#[pyclass]` wrapper for a `batchalign_core::Backend` impl
/// with a no-arg `new()` constructor.
///
/// Example:
/// ```ignore
/// native_backend!(CompareBackend, "CompareBackend", ::batchalign_core::backends::CompareBackend);
/// ```
///
/// The generated class:
/// - constructs via `<core>::new()` from Python (`CompareBackend()`),
/// - exposes `name`, `batch_policy`, `tasks` getters reading the trait,
/// - exposes `__batchalign_native_arc__` returning a `PyCapsule` that
///   carries an `Arc<dyn Backend>`. The engine reads this in
///   `BackendImpl::from_py` and routes the call natively.
#[macro_export]
macro_rules! native_backend {
    ($wrapper:ident, $py_name:literal, $core:path) => {
        #[pyo3::pyclass(name = $py_name, module = "batchalign._core.backends")]
        #[derive(Clone)]
        pub struct $wrapper {
            inner: ::std::sync::Arc<$core>,
        }

        impl $wrapper {
            #[allow(dead_code)]
            pub fn as_backend(&self) -> ::std::sync::Arc<dyn ::batchalign_core::Backend> {
                self.inner.clone()
            }
        }

        #[pyo3::pymethods]
        impl $wrapper {
            #[new]
            fn py_new() -> Self {
                Self {
                    inner: ::std::sync::Arc::new(<$core>::new()),
                }
            }

            /// Carries an `Arc<dyn Backend>` to the engine. Presence of
            /// this attribute is how `BackendImpl::from_py` detects a
            /// native wrapper; the capsule pointee is cloned out and
            /// stored as `BackendImpl::Native`.
            #[getter]
            fn __batchalign_native_arc__<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCapsule>> {
                let arc: ::std::sync::Arc<dyn ::batchalign_core::Backend> = self.inner.clone();
                let name =
                    ::std::ffi::CString::new($crate::native_backends::NATIVE_ARC_CAPSULE_NAME)
                        .expect("static capsule name");
                pyo3::types::PyCapsule::new(py, arc, Some(name))
            }

            #[getter]
            fn name(&self) -> &str {
                ::batchalign_core::Backend::name(&*self.inner)
            }

            #[getter]
            fn batch_policy(&self) -> ::batchalign_core::BatchPolicy {
                ::batchalign_core::Backend::batch_policy(&*self.inner)
            }

            #[getter]
            fn tasks(&self) -> Vec<::batchalign_core::Task> {
                ::batchalign_core::Backend::tasks(&*self.inner).to_vec()
            }

            fn __repr__(&self) -> String {
                format!(
                    "{}(name={:?})",
                    $py_name,
                    ::batchalign_core::Backend::name(&*self.inner)
                )
            }
        }
    };
}
