//! The [`ParseError`] diagnostic type with source-backed context for `miette` rendering.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::Span;
use super::codes::ErrorCode;
use super::context::ErrorContext;
use super::source_location::{ErrorLabel, Severity, SourceLocation};
use miette::Diagnostic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::OnceLock;
use thiserror::Error;

use super::builder::ParseErrorBuilder;

// Custom SourceCode implementation that supports line offsets for miette.
//
// Stores the raw source text and a line offset. When miette calls `read_span`,
// we delegate to the raw text and then adjust the starting line number in the
// returned `SpanContents`. This avoids the fragile "prepend newlines" hack
// that required coordinating span adjustments across three separate code paths.
/// Source-code wrapper that preserves original file line numbering for snippets.
pub(crate) struct SourceCodeWithOffset {
    name: String,
    source: String,
    /// 1-indexed line number where `source` starts in the original file
    line_offset: usize,
}

impl SourceCodeWithOffset {
    /// Create a source wrapper with a 1-indexed starting line offset.
    fn new(name: impl Into<String>, source: impl Into<String>, line_offset: usize) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            line_offset,
        }
    }
}

impl miette::SourceCode for SourceCodeWithOffset {
    /// Read a span while adjusting line numbers back to original-file coordinates.
    fn read_span<'a>(
        &'a self,
        span: &miette::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn miette::SpanContents<'a> + 'a>, miette::MietteError> {
        // Delegate to the raw source bytes for span extraction and context line collection
        let raw_contents =
            self.source
                .as_bytes()
                .read_span(span, context_lines_before, context_lines_after)?;

        // Adjust line number: raw_contents.line() is 0-indexed relative to our snippet,
        // line_offset is 1-indexed in the original file, so the display line is:
        //   (line_offset - 1) + raw_line  (0-indexed for MietteSpanContents)
        let adjusted_line = self.line_offset.saturating_sub(1) + raw_contents.line();

        Ok(Box::new(miette::MietteSpanContents::new_named(
            self.name.clone(),
            raw_contents.data(),
            *raw_contents.span(),
            adjusted_line,
            raw_contents.column(),
            raw_contents.line_count(),
        )))
    }
}

/// Structured parse/validation diagnostic with optional source-backed context.
#[derive(Error, Serialize, Deserialize, JsonSchema)]
pub struct ParseError {
    /// Error code (e.g., "E001", "W042")
    pub code: ErrorCode,
    /// Error severity
    pub severity: Severity,
    /// Source location
    pub location: SourceLocation,
    /// Rich context for the error (optional - None when source not available during validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ErrorContext>,
    /// Optional secondary labels for multi-span diagnostics
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub labels: Vec<ErrorLabel>,
    /// Human-readable error message (plain language)
    pub message: String,
    /// Optional suggestion for how to fix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Optional help URL (defaults to documentation_url from code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
    /// Cached source code for miette display (not serialized, lazy-initialized)
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) source_cache: OnceLock<SourceCodeWithOffset>,
}

// Manual trait implementations that skip source_cache (not part of equality/cloning)
impl Clone for ParseError {
    /// Clone user-visible fields while resetting lazy source cache.
    fn clone(&self) -> Self {
        Self {
            code: self.code,
            severity: self.severity,
            location: self.location,
            context: self.context.clone(),
            labels: self.labels.clone(),
            message: self.message.clone(),
            suggestion: self.suggestion.clone(),
            help_url: self.help_url.clone(),
            source_cache: OnceLock::new(), // Always create fresh cache
        }
    }
}

impl fmt::Debug for ParseError {
    /// Debug output intentionally omits lazy cache internals.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParseError")
            .field("code", &self.code)
            .field("severity", &self.severity)
            .field("location", &self.location)
            .field("context", &self.context)
            .field("labels", &self.labels)
            .field("message", &self.message)
            .field("suggestion", &self.suggestion)
            .field("help_url", &self.help_url)
            // Skip source_cache from debug output
            .finish()
    }
}

impl PartialEq for ParseError {
    /// Equality compares semantic diagnostic fields and ignores lazy cache state.
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.severity == other.severity
            && self.location == other.location
            && self.context == other.context
            && self.labels == other.labels
            && self.message == other.message
            && self.suggestion == other.suggestion
            && self.help_url == other.help_url
        // Skip source_cache from equality
    }
}

impl ParseError {
    /// Create a new parse error with all required fields.
    ///
    /// This is the most general constructor. Prefer [`at_span`](Self::at_span) or
    /// [`from_source_span`](Self::from_source_span) for the common cases, or
    /// [`build`](Self::build) for the builder pattern.
    ///
    /// # Parameters
    ///
    /// - `code`: The error code identifying the kind of diagnostic (e.g., `ErrorCode::MissingTerminator`).
    /// - `severity`: Whether this is an error or a warning.
    /// - `location`: The source location (byte span, optional line/column).
    /// - `context`: Optional rich context for rendering source snippets. Pass `None`
    ///   when source text is not available (e.g., during post-parse validation).
    /// - `message`: Human-readable description of the problem.
    ///
    /// # Returns
    ///
    /// A fully constructed `ParseError` with `help_url` auto-populated from the error code's
    /// documentation URL, and no suggestion or secondary labels.
    pub fn new(
        code: ErrorCode,
        severity: Severity,
        location: SourceLocation,
        context: impl Into<Option<ErrorContext>>,
        message: impl Into<String>,
    ) -> Self {
        let help_url = Some(code.documentation_url());
        Self {
            code,
            severity,
            location,
            context: context.into(),
            labels: Vec::new(),
            message: message.into(),
            suggestion: None,
            help_url,
            source_cache: OnceLock::new(),
        }
    }

    /// Create an error anchored to a byte span without source context.
    ///
    /// This is the preferred constructor when source text is not available for
    /// snippet rendering (e.g., during validation passes that only have the AST).
    /// The resulting error will display byte offsets but no source-code snippet.
    ///
    /// # Parameters
    ///
    /// - `code`: The error code identifying the kind of diagnostic.
    /// - `severity`: Whether this is an error or a warning.
    /// - `span`: Byte range in the source file where the problem occurred.
    /// - `message`: Human-readable description of the problem.
    pub fn at_span(
        code: ErrorCode,
        severity: Severity,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            code,
            severity,
            SourceLocation::new(span),
            Option::<ErrorContext>::None,
            message,
        )
    }

    /// Create an error anchored to a byte span with source-backed context.
    ///
    /// This is the preferred constructor during parsing, where the source text
    /// is available for rendering rich `miette` snippets with underlined spans.
    ///
    /// # Parameters
    ///
    /// - `code`: The error code identifying the kind of diagnostic.
    /// - `severity`: Whether this is an error or a warning.
    /// - `span`: Byte range in the source file where the problem occurred.
    /// - `source_text`: The surrounding source text for snippet rendering (typically
    ///   the full line or a few lines around the error).
    /// - `offending_text`: The specific text within `source_text` that caused the
    ///   error, used as the primary label in rendered diagnostics.
    /// - `message`: Human-readable description of the problem.
    pub fn from_source_span(
        code: ErrorCode,
        severity: Severity,
        span: Span,
        source_text: impl Into<String>,
        offending_text: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            code,
            severity,
            SourceLocation::new(span),
            ErrorContext::new(source_text, span, offending_text),
            message,
        )
    }

    /// Start building a new parse error with ergonomic builder pattern.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::{ParseError, ErrorCode, Severity, ParseErrorBuilderError};
    ///
    /// # fn build_error() -> Result<ParseError, ParseErrorBuilderError> {
    /// let error = ParseError::build(ErrorCode::InvalidMediaBullet)
    ///     .severity(Severity::Error)
    ///     .at(10, 20)
    ///     .message("Invalid media bullet format")
    ///     .suggestion("Use format: ·start_end·")
    ///     .finish()?;
    /// # Ok(error)
    /// # }
    /// ```
    pub fn build(code: ErrorCode) -> ParseErrorBuilder {
        ParseErrorBuilder::new(code)
    }

    /// Create a simple internal error for unexpected conditions.
    ///
    /// Use this for internal parser errors that indicate bugs or
    /// unimplemented features rather than user-facing CHAT errors.
    /// The error code is always [`ErrorCode::TreeParsingError`] with
    /// [`Severity::Error`] and no source context.
    ///
    /// # Parameters
    ///
    /// - `message`: Description of the internal problem.
    /// - `span`: Byte range where the unexpected condition occurred.
    pub fn internal(message: impl Into<String>, span: Span) -> Self {
        Self::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::new(span),
            None,
            message,
        )
    }

    /// Add a suggestion for how to fix this error.
    ///
    /// The suggestion is displayed as a `help:` line in `miette` output.
    /// Returns `self` for method chaining.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a secondary label pointing to a related source span.
    ///
    /// Secondary labels appear as additional underlined spans in `miette`
    /// output, providing context such as "first declared here" or
    /// "expected type from this". Returns `self` for method chaining.
    pub fn with_label(mut self, label: ErrorLabel) -> Self {
        self.labels.push(label);
        self
    }

    /// Override the help URL that links to documentation for this error.
    ///
    /// By default, the URL is derived from the error code's
    /// [`documentation_url()`](ErrorCode::documentation_url). Use this method
    /// to point to a more specific page. Returns `self` for method chaining.
    pub fn with_help_url(mut self, url: impl Into<String>) -> Self {
        self.help_url = Some(url.into());
        self
    }
}

impl fmt::Display for ParseError {
    /// Render a concise diagnostic summary for logs/CLI output.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Show line:column if available, otherwise show byte offsets
        match (self.location.line, self.location.column) {
            (Some(line), Some(column)) => write!(
                f,
                "{}[{}]: {} (line {}, column {}, bytes {}..{})",
                self.severity,
                self.code,
                self.message,
                line,
                column,
                self.location.span.start,
                self.location.span.end
            ),
            _ => write!(
                f,
                "{}[{}]: {} (bytes {}..{})",
                self.severity,
                self.code,
                self.message,
                self.location.span.start,
                self.location.span.end
            ),
        }
    }
}

impl Diagnostic for ParseError {
    /// Expose machine-readable diagnostic code to `miette`.
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(self.code.as_str()))
    }

    /// Expose severity level to `miette`.
    fn severity(&self) -> Option<miette::Severity> {
        Some(self.severity.into())
    }

    /// Expose suggestion text as optional help message.
    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.suggestion
            .as_ref()
            .map(|s| Box::new(s) as Box<dyn std::fmt::Display>)
    }

    /// Expose documentation URL when available.
    fn url<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.help_url
            .as_ref()
            .map(|u| Box::new(u) as Box<dyn std::fmt::Display>)
    }

    /// Provide primary and secondary labeled spans for rich diagnostics.
    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        let ctx = self.context.as_ref()?;

        let mut labels = Vec::with_capacity(1 + self.labels.len());

        // Primary location — use context.span (relative to source_text)
        // not location.span (absolute file offsets).
        // No offset adjustment needed: SourceCodeWithOffset handles line
        // numbering natively via read_span, so spans are used as-is.
        labels.push(miette::LabeledSpan::new_with_span(
            Some("here".to_string()),
            ctx.span,
        ));

        // Secondary labels — spans are relative to source_text (set by enhance_errors_with_source)
        for label in &self.labels {
            labels.push(miette::LabeledSpan::new_with_span(
                Some(label.message.clone()),
                label.span,
            ));
        }

        Some(Box::new(labels.into_iter()))
    }

    /// Provide source text/snippet provider for `miette` rendering.
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        let ctx = self.context.as_ref()?;

        if ctx.source_text.is_empty() {
            return None;
        }

        // Lazily initialize SourceCodeWithOffset with proper line numbering
        // DEFAULT: Report line numbers as 1-based when no offset is provided.
        let line_offset = ctx.line_offset.unwrap_or(1);
        let source = self.source_cache.get_or_init(|| {
            SourceCodeWithOffset::new("input", ctx.source_text.clone(), line_offset)
        });

        Some(source as &dyn miette::SourceCode)
    }
}
