//! Error-sink adapter that applies `ValidationConfig` overrides/filtering.
//!
//! This wrapper keeps parser code policy-agnostic: parsers emit canonical
//! diagnostics, and policy decisions (strict/lenient/remapped severity) are
//! handled at sink boundaries.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::{ErrorSink, ParseError};
use crate::ValidationConfig;

/// Error sink wrapper that applies `ValidationConfig` before forwarding.
///
/// This sink intercepts errors and:
/// - Applies severity overrides from ValidationConfig
/// - Filters out disabled error codes
/// - Forwards modified errors to the wrapped sink
///
/// # Example
///
/// ```
/// use talkbank_model::{ErrorCollector, ConfigurableErrorSink};
/// use talkbank_model::ValidationConfig;
/// use talkbank_model::{ErrorCode, Severity};
///
/// let config = ValidationConfig::new()
///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
///     .disable(ErrorCode::InvalidOverlapIndex);
///
/// let inner = ErrorCollector::new();
/// let sink = ConfigurableErrorSink::new(&inner, config);
///
/// // Errors are filtered/modified before reaching inner sink
/// ```
pub struct ConfigurableErrorSink<'a, S: ErrorSink> {
    inner: &'a S,
    config: ValidationConfig,
}

impl<'a, S: ErrorSink> ConfigurableErrorSink<'a, S> {
    /// Create a new configurable sink around an existing sink implementation.
    ///
    /// # Parameters
    ///
    /// * `inner` - The wrapped error sink that will receive filtered/modified errors
    /// * `config` - Validation configuration specifying severity overrides and disabled codes
    pub fn new(inner: &'a S, config: ValidationConfig) -> Self {
        Self { inner, config }
    }

    /// Return the wrapped sink receiving post-configuration diagnostics.
    pub fn inner(&self) -> &S {
        self.inner
    }

    /// Return the active validation configuration.
    pub fn config(&self) -> &ValidationConfig {
        &self.config
    }

    /// Apply severity overrides and disabled-code filtering to one diagnostic.
    ///
    /// Returns `None` when the diagnostic is disabled by configuration.
    fn apply_config(&self, mut error: ParseError) -> Option<ParseError> {
        match self.config.effective_severity(error.code, error.severity) {
            Some(new_severity) => {
                error.severity = new_severity;
                Some(error)
            }
            None => None, // Error is disabled
        }
    }
}

impl<'a, S: ErrorSink> ErrorSink for ConfigurableErrorSink<'a, S> {
    /// Report one diagnostic after applying configuration rules.
    fn report(&self, error: ParseError) {
        if let Some(modified_error) = self.apply_config(error) {
            self.inner.report(modified_error);
        }
    }

    /// Report a batch of diagnostics after filtering/re-mapping severities.
    fn report_all(&self, errors: Vec<ParseError>) {
        let filtered: Vec<ParseError> = errors
            .into_iter()
            .filter_map(|e| self.apply_config(e))
            .collect();

        if !filtered.is_empty() {
            self.inner.report_all(filtered);
        }
    }

    /// Report a smallvec-backed batch of diagnostics.
    fn report_vec(&self, errors: crate::ErrorVec) {
        self.report_all(errors.into_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, ErrorCollector, ErrorContext, Severity, SourceLocation};

    /// Builds test error.
    fn make_test_error(code: ErrorCode, severity: Severity) -> ParseError {
        ParseError::new(
            code,
            severity,
            SourceLocation::at_offset(0),
            ErrorContext::new("test", 0..4, "test"),
            "Test error",
        )
    }

    #[test]
    fn test_downgrade_error() {
        let config =
            ValidationConfig::new().downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);

        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, config);

        let error = make_test_error(ErrorCode::IllegalUntranscribed, Severity::Error);
        sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Warning);
    }

    #[test]
    fn test_disable_error() {
        let config = ValidationConfig::new().disable(ErrorCode::InvalidOverlapIndex);

        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, config);

        let error = make_test_error(ErrorCode::InvalidOverlapIndex, Severity::Error);
        sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 0, "Disabled error should not be reported");
    }

    #[test]
    fn test_upgrade_warning() {
        let config = ValidationConfig::new().upgrade(ErrorCode::UnknownAnnotation, Severity::Error);

        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, config);

        let error = make_test_error(ErrorCode::UnknownAnnotation, Severity::Warning);
        sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn test_no_override_uses_original() {
        let config = ValidationConfig::new();

        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, config);

        let error = make_test_error(ErrorCode::IllegalUntranscribed, Severity::Error);
        sink.report(error);

        let errors = inner.into_vec();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn test_report_all_filters_disabled() {
        let config = ValidationConfig::new().disable(ErrorCode::InvalidOverlapIndex);

        let inner = ErrorCollector::new();
        let sink = ConfigurableErrorSink::new(&inner, config);

        let errors = vec![
            make_test_error(ErrorCode::IllegalUntranscribed, Severity::Error),
            make_test_error(ErrorCode::InvalidOverlapIndex, Severity::Error), // Disabled
            make_test_error(ErrorCode::UnknownAnnotation, Severity::Warning),
        ];

        sink.report_all(errors);

        let result_errors = inner.into_vec();
        assert_eq!(result_errors.len(), 2, "Should filter out disabled error");
        assert_eq!(result_errors[0].code, ErrorCode::IllegalUntranscribed);
        assert_eq!(result_errors[1].code, ErrorCode::UnknownAnnotation);
    }
}
