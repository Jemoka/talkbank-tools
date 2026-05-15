//! Error sink adapter that duplicates diagnostics to two downstream sinks.
//!
//! This adapter is useful when one caller needs both streaming behavior and a
//! local collector. A common pattern is to stream adjusted errors to the user
//! while also collecting them to decide whether a parse step should be rejected.

use crate::{ErrorSink, ParseError};

/// Error sink that forwards each diagnostic to two downstream sinks.
pub struct TeeErrorSink<'a, Primary: ErrorSink + ?Sized, Secondary: ErrorSink + ?Sized> {
    primary: &'a Primary,
    secondary: &'a Secondary,
}

impl<'a, Primary: ErrorSink + ?Sized, Secondary: ErrorSink + ?Sized>
    TeeErrorSink<'a, Primary, Secondary>
{
    /// Create a tee sink that forwards errors to both downstream sinks.
    pub fn new(primary: &'a Primary, secondary: &'a Secondary) -> Self {
        Self { primary, secondary }
    }
}

impl<Primary: ErrorSink + ?Sized, Secondary: ErrorSink + ?Sized> ErrorSink
    for TeeErrorSink<'_, Primary, Secondary>
{
    /// Forward each error to both downstream sinks.
    fn report(&self, error: ParseError) {
        self.primary.report(error.clone());
        self.secondary.report(error);
    }
}

#[cfg(test)]
mod tests {
    //! Tee sink tests.

    use crate::{
        ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation,
        TeeErrorSink,
    };

    /// Tee sinks should forward diagnostics to both downstream sinks.
    #[test]
    fn tee_sink_reports_to_both_downstreams() {
        let primary = ErrorCollector::new();
        let secondary = ErrorCollector::new();
        let tee = TeeErrorSink::new(&primary, &secondary);

        tee.report(ParseError::new(
            ErrorCode::InternalError,
            Severity::Error,
            SourceLocation::from_offsets(0, 1),
            ErrorContext::new("test", 0..1, "t"),
            "test error",
        ));

        assert_eq!(primary.len(), 1);
        assert_eq!(secondary.len(), 1);
    }
}
