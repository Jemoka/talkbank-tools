//! Builder pattern for constructing [`ParseError`] instances ergonomically.

use super::Span;
use super::codes::ErrorCode;
use super::context::ErrorContext;
use super::parse_error::ParseError;
use super::source_location::{ErrorLabel, Severity, SourceLocation};
use std::sync::OnceLock;

/// Builder for constructing ParseError instances ergonomically.
///
/// Created via `ParseError::build(code)`. Provides a fluent API for
/// setting error properties without the verbose constructor.
///
/// # Required Fields
///
/// The following must be set before calling `finish()`:
/// - `message` - Human-readable error description
///
/// # Optional Fields with Defaults
///
/// - `severity` - Defaults to `Severity::Error`
/// - `location` - Required
/// - `context` - Defaults to empty context
/// - `suggestion` - No suggestion by default
///
/// # Example
///
/// ```
/// use talkbank_model::{ParseError, ErrorCode, Severity, ParseErrorBuilderError};
///
/// # fn build_examples() -> Result<(), ParseErrorBuilderError> {
/// // Minimal usage
/// let _error = ParseError::build(ErrorCode::InvalidMediaBullet)
///     .at(0, 1)
///     .message("Invalid format")
///     .finish()?;
///
/// // Full usage
/// let _error = ParseError::build(ErrorCode::MissingTerminator)
///     .severity(Severity::Error)
///     .at(100, 110)
///     .context_from_source("hello world", 100..110, "hello")
///     .message("Missing utterance terminator")
///     .suggestion("Add a period, question mark, or exclamation point")
///     .finish()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ParseErrorBuilder {
    code: ErrorCode,
    severity: Severity,
    location: Option<SourceLocation>,
    context: Option<ErrorContext>,
    message: Option<String>,
    suggestion: Option<String>,
    labels: Vec<ErrorLabel>,
}

/// Errors that can occur when building a ParseError.
#[derive(Debug, thiserror::Error)]
pub enum ParseErrorBuilderError {
    /// The builder was finished without setting a message.
    #[error("ParseErrorBuilder requires a message - call .message() before .finish()")]
    MissingMessage,
    /// The builder was finished without setting a location.
    #[error(
        "ParseErrorBuilder requires a location - call .at(), .at_span(), or .location() before .finish()"
    )]
    MissingLocation,
}

impl ParseErrorBuilder {
    /// Create a new builder with the given error code.
    pub(crate) fn new(code: ErrorCode) -> Self {
        Self {
            code,
            severity: Severity::Error,
            location: None,
            context: None,
            message: None,
            suggestion: None,
            labels: Vec::new(),
        }
    }

    /// Set the error severity (default: Error).
    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the error location from byte offsets.
    pub fn at(mut self, start: usize, end: usize) -> Self {
        self.location = Some(SourceLocation::from_offsets(start, end));
        self
    }

    /// Set the error location from a Span.
    pub fn at_span(mut self, span: Span) -> Self {
        self.location = Some(SourceLocation::new(span));
        self
    }

    /// Set the error location from a SourceLocation.
    pub fn location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Set the error context directly.
    pub fn context(mut self, context: ErrorContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Set the error context from source text components.
    ///
    /// # Arguments
    ///
    /// * `source_text` - The source code containing the error
    /// * `span` - Byte range of the error within source_text
    /// * `offending_text` - The specific text that caused the error
    pub fn context_from_source(
        mut self,
        source_text: impl Into<String>,
        span: std::ops::Range<usize>,
        offending_text: impl Into<String>,
    ) -> Self {
        self.context = Some(ErrorContext::new(source_text, span, offending_text));
        self
    }

    /// Set the human-readable error message (required).
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set a suggestion for how to fix the error.
    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a secondary label span.
    pub fn label(mut self, label: ErrorLabel) -> Self {
        self.labels.push(label);
        self
    }

    /// Build the ParseError, consuming the builder.
    pub fn finish(self) -> Result<ParseError, ParseErrorBuilderError> {
        let message = match self.message {
            Some(message) => message,
            None => return Err(ParseErrorBuilderError::MissingMessage),
        };

        let location = match self.location {
            Some(location) => location,
            None => return Err(ParseErrorBuilderError::MissingLocation),
        };

        let help_url = Some(self.code.documentation_url());

        Ok(ParseError {
            code: self.code,
            severity: self.severity,
            location,
            context: self.context,
            labels: self.labels,
            message,
            suggestion: self.suggestion,
            help_url,
            source_cache: OnceLock::new(),
        })
    }

    /// Build the ParseError, returning None if required fields are missing.
    ///
    /// This is the non-panicking alternative to `finish()`.
    pub fn try_finish(self) -> Option<ParseError> {
        let message = self.message?;

        let location = self.location?;

        let help_url = Some(self.code.documentation_url());

        Some(ParseError {
            code: self.code,
            severity: self.severity,
            location,
            context: self.context,
            labels: self.labels,
            message,
            suggestion: self.suggestion,
            help_url,
            source_cache: OnceLock::new(),
        })
    }
}
