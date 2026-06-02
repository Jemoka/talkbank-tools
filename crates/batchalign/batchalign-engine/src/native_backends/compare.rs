//! `batchalign._core.backends.CompareBackend` — pyo3 wrapper around
//! `batchalign_core::backends::CompareBackend`. See the parent module for
//! how the wrapper is generated and routed.

use crate::native_backend;

native_backend!(
    CompareBackend,
    "CompareBackend",
    ::batchalign_core::backends::CompareBackend
);
