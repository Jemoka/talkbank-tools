//! Typed boundary errors for the Rust ↔ Python PyO3 interface.
//!
//! See `book/src/batchalign/architecture/python-rust-errors.md` for the
//! contract: every error crossing the boundary is categorized into one
//! of the variants of [`BatchalignBoundaryError`], which converts to
//! the matching Python exception subclass (declared via
//! [`pyo3::create_exception!`]) instead of the legacy
//! `PyValueError(error.to_string())` shape that discards provenance.
//!
//! The variants line up 1:1 with the exception classes Python imports
//! from `batchalign_core`. Adding a new error category means adding a
//! new variant *and* a matching `create_exception!` declaration.

use std::path::PathBuf;

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// Python exception hierarchy. `BatchalignError` is the common ancestor
// so call sites can `except BatchalignError` to catch any boundary
// failure regardless of category. `SkipFileWarning` deliberately does
// not inherit from `Warning` — see the design doc's Phase D1 decisions
// for why; the existing Python code raises and catches it as an
// exception, not a warning.
create_exception!(
    batchalign_core,
    BatchalignError,
    PyException,
    "Common ancestor for every typed exception raised across the \
     Rust/Python PyO3 boundary. Catch this to handle any boundary \
     failure regardless of category."
);
create_exception!(
    batchalign_core,
    CHATValidationException,
    BatchalignError,
    "Raised when CHAT validation detects structural problems. \
     Carries `errors: list[ValidationErrorEntry]` and an optional \
     `bug_report_id: str`."
);
create_exception!(
    batchalign_core,
    DocumentValidationException,
    BatchalignError,
    "Raised when validating a non-CHAT document payload fails."
);
create_exception!(
    batchalign_core,
    ConfigNotFoundError,
    BatchalignError,
    "Raised when required Batchalign config files are missing from \
     disk. Carries `path: str`."
);
create_exception!(
    batchalign_core,
    ConfigError,
    BatchalignError,
    "Raised for syntactically present but semantically invalid \
     configuration."
);
create_exception!(
    batchalign_core,
    PayloadTooLargeError,
    BatchalignError,
    "Raised when an HTTP request body exceeds the configured limit. \
     Carries `limit_layer: str` (\"inner\" or \"outer\") and \
     `configured_bytes: int`."
);
create_exception!(
    batchalign_core,
    SkipFileWarning,
    PyException,
    "Signals that a file should be skipped with a warning, not \
     failed. Carries the optional raw `chat_text: str` to copy to \
     output unchanged."
);

/// One structured CHAT-validation entry surfaced through
/// [`BatchalignBoundaryError::ChatValidation`]. Mirrors the
/// `ValidationErrorEntry` TypedDict on the Python side so Python catch
/// sites see the same field shape regardless of whether the error
/// originated in pure-Python code or crossed the PyO3 boundary.
#[derive(Debug, Clone)]
pub(crate) struct ValidationErrorEntry {
    pub code: String,
    pub severity: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
    pub suggestion: Option<String>,
}

impl ValidationErrorEntry {
    fn into_pydict<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("code", self.code)?;
        dict.set_item("severity", self.severity)?;
        if let Some(line) = self.line {
            dict.set_item("line", line)?;
        }
        if let Some(column) = self.column {
            dict.set_item("column", column)?;
        }
        dict.set_item("message", self.message)?;
        if let Some(suggestion) = self.suggestion {
            dict.set_item("suggestion", suggestion)?;
        }
        Ok(dict)
    }
}

/// Which body-limit layer rejected an oversized request. Carried by
/// [`BatchalignBoundaryError::PayloadTooLarge`] so the Python side
/// (and downstream tracing / dashboards) can distinguish the inner
/// (axum `Json` extractor) limit from the outer
/// (`RequestBodyLimitLayer`) limit without grepping the message text.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Awaiting first call site in batchalign's payload-too-large path.
pub(crate) enum BodyLimitLayer {
    Inner,
    Outer,
}

impl BodyLimitLayer {
    fn as_str(self) -> &'static str {
        match self {
            BodyLimitLayer::Inner => "inner",
            BodyLimitLayer::Outer => "outer",
        }
    }
}

/// Typed boundary error. Every variant maps to exactly one Python
/// exception subclass. The category names line up with the Python
/// hierarchy; adding a category here requires a matching
/// [`create_exception!`] declaration above and a Python-side import.
#[derive(Debug)]
#[allow(dead_code)] // Most variants are awaiting their first call site as Phase D sweeps the rest of pyo3/.
pub(crate) enum BatchalignBoundaryError {
    /// CHAT validation produced a structured error list. Maps to
    /// `CHATValidationException` on the Python side; the entries +
    /// optional `bug_report_id` populate fields on the raised
    /// exception so Python catch sites can `exc.errors[i].code`
    /// directly.
    ChatValidation {
        message: String,
        entries: Vec<ValidationErrorEntry>,
        bug_report_id: Option<String>,
    },

    /// A non-CHAT document payload (server's content-mode submission)
    /// failed validation. Maps to `DocumentValidationException`.
    DocumentValidation { message: String },

    /// A required config file or value was missing on disk. Maps to
    /// `ConfigNotFoundError`.
    ConfigNotFound { path: PathBuf },

    /// A config file was syntactically present but semantically
    /// invalid. Maps to `ConfigError`.
    ConfigInvalid { message: String },

    /// The PyO3 boundary's body limit rejected the request. Maps to
    /// `PayloadTooLargeError` and carries which layer fired.
    PayloadTooLarge {
        limit_layer: BodyLimitLayer,
        configured_bytes: u64,
    },

    /// The file should pass through unchanged with a warning logged.
    /// Maps to `SkipFileWarning` and carries the raw CHAT text.
    SkipFileWarning {
        message: String,
        chat_text: Option<String>,
    },

    /// Any other Rust-side failure that doesn't fit the typed buckets
    /// above. Maps to a `BatchalignError` (the Python parent class).
    /// Not a fallback to `PyValueError`: the typed parent gives
    /// Python catch sites a stable common ancestor for boundary
    /// failures.
    Internal { message: String },
}

impl BatchalignBoundaryError {
    /// Construct an `Internal` error from any displayable value.
    /// Convenience for the most common pilot call site —
    /// `serde_json::from_value(...).map_err(BatchalignBoundaryError::internal)?`
    /// produces a typed boundary error that propagates through `?`.
    pub fn internal(source: impl std::fmt::Display) -> Self {
        Self::Internal {
            message: source.to_string(),
        }
    }

    /// Convert into a typed `PyErr`. Equivalent to `.into()` but with
    /// the target type pinned so it works inside `map_err` closures
    /// where rustc can't infer the conversion target from context.
    pub fn into_py_err(self) -> PyErr {
        self.into()
    }
}

impl std::fmt::Display for BatchalignBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChatValidation { message, .. } => write!(f, "{message}"),
            Self::DocumentValidation { message } => write!(f, "{message}"),
            Self::ConfigNotFound { path } => write!(f, "config not found: {}", path.display()),
            Self::ConfigInvalid { message } => write!(f, "{message}"),
            Self::PayloadTooLarge {
                limit_layer,
                configured_bytes,
            } => write!(
                f,
                "payload too large: {} layer rejected request (limit: {} bytes)",
                limit_layer.as_str(),
                configured_bytes,
            ),
            Self::SkipFileWarning { message, .. } => write!(f, "{message}"),
            Self::Internal { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for BatchalignBoundaryError {}

impl From<BatchalignBoundaryError> for PyErr {
    fn from(error: BatchalignBoundaryError) -> Self {
        // The `From` impl runs without the GIL held by the caller in
        // most code paths (PyO3 picks it up automatically on `?`-style
        // propagation), so we acquire one here for any kwargs setup
        // and exception construction.
        let message = error.to_string();
        match error {
            BatchalignBoundaryError::ChatValidation {
                entries,
                bug_report_id,
                ..
            } => Python::attach(|py| -> PyErr {
                let py_err = CHATValidationException::new_err(message);
                let py_entries: Vec<Bound<'_, pyo3::types::PyDict>> = match entries
                    .into_iter()
                    .map(|entry| entry.into_pydict(py))
                    .collect::<PyResult<Vec<_>>>()
                {
                    Ok(v) => v,
                    Err(setup_err) => return setup_err,
                };
                let value = py_err.value(py);
                if let Err(e) = value.setattr("errors", py_entries) {
                    return e;
                }
                if let Err(e) = value.setattr("bug_report_id", bug_report_id) {
                    return e;
                }
                py_err
            }),
            BatchalignBoundaryError::DocumentValidation { .. } => {
                DocumentValidationException::new_err(message)
            }
            BatchalignBoundaryError::ConfigNotFound { path } => Python::attach(|py| -> PyErr {
                let py_err = ConfigNotFoundError::new_err(message);
                let value = py_err.value(py);
                if let Err(e) = value.setattr("path", path.to_string_lossy().into_owned()) {
                    return e;
                }
                py_err
            }),
            BatchalignBoundaryError::ConfigInvalid { .. } => ConfigError::new_err(message),
            BatchalignBoundaryError::PayloadTooLarge {
                limit_layer,
                configured_bytes,
            } => Python::attach(|py| -> PyErr {
                let py_err = PayloadTooLargeError::new_err(message);
                let value = py_err.value(py);
                if let Err(e) = value.setattr("limit_layer", limit_layer.as_str()) {
                    return e;
                }
                if let Err(e) = value.setattr("configured_bytes", configured_bytes) {
                    return e;
                }
                py_err
            }),
            BatchalignBoundaryError::SkipFileWarning { chat_text, .. } => {
                Python::attach(|py| -> PyErr {
                    let py_err = SkipFileWarning::new_err(message);
                    let value = py_err.value(py);
                    if let Err(e) = value.setattr("chat_text", chat_text) {
                        return e;
                    }
                    py_err
                })
            }
            BatchalignBoundaryError::Internal { .. } => BatchalignError::new_err(message),
        }
    }
}
