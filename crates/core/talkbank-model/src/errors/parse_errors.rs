//! [`ParseErrors`] collection type and [`ParseResult`] type alias.

use super::parse_error::ParseError;
use super::source_location::{ErrorVec, Severity};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Owned collection of parse/validation diagnostics.
///
/// Unlike [`ErrorCollector`](crate::ErrorCollector), this is a plain data container
/// without interior mutability -- suitable for returning from parse functions and
/// serializing as part of cached results.
#[derive(Error, Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParseErrors {
    /// All errors collected during parsing.
    pub errors: Vec<ParseError>,
}

impl ParseErrors {
    /// Create an empty error collection.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add an error to the collection
    pub fn push(&mut self, error: ParseError) {
        self.errors.push(error);
    }

    /// Add multiple errors from another ParseErrors collection
    pub fn extend(&mut self, other: ParseErrors) {
        self.errors.extend(other.errors);
    }

    /// Add multiple errors from an ErrorVec
    pub fn extend_from_vec(&mut self, errors: ErrorVec) {
        self.errors.extend(errors);
    }

    /// Check if there are any errors
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the number of errors
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Convert into an `ErrorVec` (smallvec-optimized) without cloning.
    pub fn into_error_vec(self) -> ErrorVec {
        self.errors.into_iter().collect()
    }

    /// Clone the contained errors into an `ErrorVec`.
    pub fn to_error_vec(&self) -> ErrorVec {
        self.errors.iter().cloned().collect()
    }

    /// Partition into errors and warnings by severity.
    ///
    /// Returns `(errors, warnings)` where each vector contains references
    /// to the diagnostics of the corresponding severity.
    pub fn errors_and_warnings(&self) -> (Vec<&ParseError>, Vec<&ParseError>) {
        let (errors, warnings): (Vec<_>, Vec<_>) = self
            .errors
            .iter()
            .partition(|e| e.severity == Severity::Error);
        (errors, warnings)
    }
}

impl Default for ParseErrors {
    /// Construct an empty error collection.
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<ParseError>> for ParseErrors {
    /// Wrap a `Vec<ParseError>` as `ParseErrors`.
    fn from(errors: Vec<ParseError>) -> Self {
        Self { errors }
    }
}

impl fmt::Display for ParseErrors {
    /// Render each contained diagnostic on its own line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for error in &self.errors {
            writeln!(f, "{}", error)?;
        }
        Ok(())
    }
}

/// Convenience type alias for parse results
pub type ParseResult<T> = Result<T, ParseErrors>;
