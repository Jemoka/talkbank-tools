//! Core diagnostic sink trait plus lightweight forwarding implementations.
//!
//! Parsers and validators stream diagnostics through [`ErrorSink`] so call sites
//! can choose how to consume them without changing parsing logic. This module
//! keeps the interface itself and the minimal stateless or forwarding sink
//! implementations that are closest to that interface.

use super::parse_error::ParseError;
use super::source_location::ErrorVec;

/// Trait for receiving errors as they are discovered during parsing or validation.
///
/// All parse and validation methods accept an `&impl ErrorSink` so that
/// diagnostics are streamed out immediately rather than accumulated in a
/// return value. This keeps parsers composable: a caller can choose to
/// collect errors (`ErrorCollector`), count them (`ParseTracker`), discard
/// them (`NullErrorSink`), or opt into feature-gated channel forwarding sinks.
///
/// Implementations must be `Send + Sync` because parsers may report
/// errors from multiple threads (e.g., during parallel file validation).
///
/// The `&T` blanket impl means shared references (`&sink`) also satisfy
/// `ErrorSink`, so callers never need to wrap sinks in `Arc`.
pub trait ErrorSink: Send + Sync {
    /// Report a single diagnostic.
    ///
    /// Called by parsers and validators as errors are discovered. The
    /// implementation decides what to do with the error (collect, count,
    /// forward, or discard).
    fn report(&self, error: ParseError);

    /// Report all errors from a `Vec`, consuming it.
    ///
    /// Default implementation calls [`report`](Self::report) in a loop.
    fn report_all(&self, errors: Vec<ParseError>) {
        for error in errors {
            self.report(error);
        }
    }

    /// Report all errors from an [`ErrorVec`] (smallvec-backed), consuming it.
    ///
    /// Default implementation calls [`report`](Self::report) in a loop.
    fn report_vec(&self, errors: ErrorVec) {
        for error in errors {
            self.report(error);
        }
    }
}

/// Error sink that forwards diagnostics through a crossbeam channel.
///
/// Used for cross-thread error streaming, e.g., when worker threads
/// parse files in parallel and a coordinator thread collects all diagnostics.
/// Send failures (disconnected receiver) are silently ignored.
#[cfg(feature = "channels")]
pub struct ChannelErrorSink {
    sender: crossbeam_channel::Sender<ParseError>,
}

#[cfg(feature = "channels")]
impl ChannelErrorSink {
    /// Wrap a crossbeam `Sender` as an error sink.
    ///
    /// The caller is responsible for creating the channel and consuming
    /// from the corresponding `Receiver`.
    pub fn new(sender: crossbeam_channel::Sender<ParseError>) -> Self {
        Self { sender }
    }
}

#[cfg(feature = "channels")]
impl ErrorSink for ChannelErrorSink {
    /// Send one error over the channel if the receiver is still alive.
    fn report(&self, error: ParseError) {
        self.sender.send(error).ok();
    }
}

/// Error sink that silently discards all reported diagnostics.
///
/// Useful in benchmarks, or when a caller only cares about the parsed
/// result and not the diagnostics (e.g., batch processing where errors
/// are expected and will be checked via the returned `ParseOutcome`).
pub struct NullErrorSink;

impl ErrorSink for NullErrorSink {
    /// Drop all reported errors.
    fn report(&self, _error: ParseError) {}
}

/// Blanket implementation for references to [`ErrorSink`].
impl<T: ErrorSink + ?Sized> ErrorSink for &T {
    /// Forward to the underlying sink.
    fn report(&self, error: ParseError) {
        (*self).report(error);
    }
}
