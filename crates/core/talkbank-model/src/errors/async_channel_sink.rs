//! Async-compatible error sink implementations.
//!
//! These sinks live in the core error subsystem because they are still just
//! diagnostic consumers. They are used by async runtimes and async-facing
//! integrations, but they are not part of validation orchestration itself.

use crate::{ErrorSink, ErrorVec, ParseError};

/// Async-compatible [`ErrorSink`] that forwards diagnostics over an MPSC channel.
///
/// Sends errors through Tokio's unbounded MPSC channel for async consumers.
/// This is useful for streaming validation results to async tasks, websocket
/// broadcasters, or other async coordination layers.
#[cfg(feature = "async")]
pub struct AsyncChannelErrorSink {
    sender: tokio::sync::mpsc::UnboundedSender<ParseError>,
}

#[cfg(feature = "async")]
impl AsyncChannelErrorSink {
    /// Create a sink that forwards parse errors to the provided channel.
    pub fn new(sender: tokio::sync::mpsc::UnboundedSender<ParseError>) -> Self {
        Self { sender }
    }
}

#[cfg(feature = "async")]
impl ErrorSink for AsyncChannelErrorSink {
    /// Send one error into the async channel.
    fn report(&self, error: ParseError) {
        let _ = self.sender.send(error);
    }

    /// Send a batch of errors into the async channel.
    fn report_all(&self, errors: Vec<ParseError>) {
        for error in errors {
            self.report(error);
        }
    }

    /// Send an [`ErrorVec`] into the async channel.
    fn report_vec(&self, errors: ErrorVec) {
        self.report_all(errors.into_vec());
    }
}

#[cfg(all(test, feature = "async"))]
mod tests {
    //! Async sink tests.

    use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

    use super::AsyncChannelErrorSink;

    /// Channel-backed async sinks should forward reported diagnostics.
    #[tokio::test]
    async fn async_channel_sink_forwards_errors() -> Result<(), String> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = AsyncChannelErrorSink::new(tx);

        sink.report(ParseError::new(
            ErrorCode::IllegalUntranscribed,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new("test", 0..4, "test"),
            "Test error",
        ));

        let received = rx
            .recv()
            .await
            .ok_or_else(|| "expected one error".to_string())?;
        assert_eq!(received.code, ErrorCode::IllegalUntranscribed);
        Ok(())
    }
}
