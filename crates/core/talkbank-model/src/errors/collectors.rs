//! In-memory diagnostic collectors and counters.
//!
//! These implementations of [`ErrorSink`](super::error_sink::ErrorSink) keep
//! diagnostics in memory or reduce them to counts. They are the default choices
//! for tests, validation passes, and parser call sites that need to inspect
//! emitted diagnostics after the fact.

use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

use super::error_sink::ErrorSink;
use super::parse_error::ParseError;
use super::source_location::{ErrorVec, Severity};

/// Lightweight error sink that counts errors and warnings without storing them.
///
/// Use this when you only need to know *whether* parsing produced diagnostics,
/// not *what* they were. The counters use relaxed atomics, so this sink is
/// safe to share across threads with minimal overhead.
pub struct ParseTracker {
    error_count: AtomicUsize,
    warning_count: AtomicUsize,
}

impl ParseTracker {
    /// Create a new parse tracker with zero counts.
    pub fn new() -> Self {
        Self {
            error_count: AtomicUsize::new(0),
            warning_count: AtomicUsize::new(0),
        }
    }

    /// Get the number of errors reported so far.
    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Get the number of warnings reported so far.
    pub fn warning_count(&self) -> usize {
        self.warning_count.load(Ordering::Relaxed)
    }

    /// Check if any errors have been reported.
    pub fn has_error(&self) -> bool {
        self.error_count() > 0
    }

    /// Check if any warnings have been reported.
    pub fn has_warning(&self) -> bool {
        self.warning_count() > 0
    }
}

impl Default for ParseTracker {
    /// Return a tracker with zero error and warning counts.
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorSink for ParseTracker {
    /// Increment the matching severity counter for one diagnostic.
    fn report(&self, error: ParseError) {
        match error.severity {
            Severity::Error => {
                self.error_count.fetch_add(1, Ordering::Relaxed);
            }
            Severity::Warning => {
                self.warning_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// In-memory collector that stores diagnostics in a `Vec<ParseError>`.
///
/// Uses `parking_lot::Mutex` for lower overhead than `std::sync::Mutex`.
///
/// ## Happy Path Optimization
///
/// The vector is lazily allocated on the first error, not at construction
/// time. This optimizes for the common case where most files have zero
/// validation errors.
pub struct ErrorCollector {
    errors: Mutex<Option<Vec<ParseError>>>,
}

impl ErrorCollector {
    /// Create a new empty error collector.
    ///
    /// No allocation occurs until the first error is reported.
    pub fn new() -> Self {
        Self {
            errors: Mutex::new(None),
        }
    }

    /// Create a collector with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            errors: Mutex::new(Some(Vec::with_capacity(capacity))),
        }
    }

    /// Get the number of collected errors.
    pub fn len(&self) -> usize {
        self.errors.lock().as_ref().map_or(0, |errors| errors.len())
    }

    /// Check if no errors have been collected.
    pub fn is_empty(&self) -> bool {
        self.errors.lock().is_none()
    }

    /// Check if any errors (not warnings) have been collected.
    pub fn has_errors(&self) -> bool {
        self.errors.lock().as_ref().is_some_and(|errors| {
            errors
                .iter()
                .any(|error| matches!(error.severity, Severity::Error))
        })
    }

    /// Consume the collector and return the collected errors.
    pub fn into_vec(self) -> Vec<ParseError> {
        self.errors.into_inner().unwrap_or_default()
    }

    /// Get a clone of the collected errors without consuming the collector.
    pub fn to_vec(&self) -> Vec<ParseError> {
        self.errors
            .lock()
            .as_ref()
            .map_or_else(Vec::new, |errors| errors.clone())
    }

    /// Convert the collected errors into an [`ErrorVec`].
    pub fn to_error_vec(&self) -> ErrorVec {
        self.errors
            .lock()
            .as_ref()
            .map_or_else(ErrorVec::new, |errors| errors.iter().cloned().collect())
    }
}

impl Default for ErrorCollector {
    /// Return an empty in-memory collector.
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorSink for ErrorCollector {
    /// Append one error to the internal buffer.
    fn report(&self, error: ParseError) {
        self.errors.lock().get_or_insert_with(Vec::new).push(error);
    }
}
