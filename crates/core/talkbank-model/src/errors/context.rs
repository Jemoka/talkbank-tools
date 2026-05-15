//! Rich source snippets and expectations attached to parse diagnostics.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Rich error context for helpful error messages
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorContext {
    /// The source text containing the error
    pub source_text: String,
    /// Byte offset span highlighting the error (relative to source_text)
    #[serde(flatten)]
    pub span: Span,
    /// What was expected at this position (multiple possibilities).
    /// SmallVec avoids heap allocation for the common case of 0–2 items.
    #[serde(skip_serializing_if = "SmallVec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub expected: SmallVec<[String; 2]>,
    /// What was actually found
    pub found: String,
    /// Starting line number of source_text in the original file (1-indexed)
    /// Used by miette to display correct line numbers in error output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_offset: Option<usize>,
}

impl ErrorContext {
    /// Create error context from source text and byte span
    pub fn new(
        source_text: impl Into<String>,
        span: impl Into<Span>,
        found: impl Into<String>,
    ) -> Self {
        Self {
            source_text: source_text.into(),
            span: span.into(),
            expected: SmallVec::new(),
            found: found.into(),
            line_offset: None,
        }
    }

    /// Create error context from reconstructed text (when original source unavailable)
    ///
    /// This is useful when validating structures that were parsed but the original
    /// source text is not available at validation time. The reconstructed text
    /// (from WriteChat serialization) is used as both source and found text.
    pub fn from_reconstructed(
        reconstructed_text: impl Into<String>,
        span: impl Into<Span>,
    ) -> Self {
        let text = reconstructed_text.into();
        Self {
            source_text: text.clone(),
            span: span.into(),
            expected: SmallVec::new(),
            found: text,
            line_offset: None,
        }
    }

    /// Set the list of expected values for this error context.
    pub fn with_expected(mut self, expected: impl Into<SmallVec<[String; 2]>>) -> Self {
        self.expected = expected.into();
        self
    }

    /// Set the line offset for miette formatting
    pub fn with_line_offset(mut self, line: usize) -> Self {
        self.line_offset = Some(line);
        self
    }

    /// Create error context from full source text and byte span, calculating line offset automatically
    ///
    /// This extracts a snippet around the span and calculates which line number it starts at
    pub fn from_source_with_span(
        full_source: &str,
        span_start: usize,
        span_end: usize,
        found: impl Into<String>,
    ) -> Self {
        use crate::SourceLocation;

        // Calculate line number for the start of the span
        let (line, _column) = SourceLocation::calculate_line_column(span_start, full_source);

        // Extract the snippet (for now, just use the span itself - could be enhanced to show context)
        let source_text = match full_source.get(span_start..span_end) {
            Some(text) => text.to_string(),
            None => String::new(),
        };

        // Create context with relative span (0..length within the snippet)
        let relative_span = Span::from_usize(0, source_text.len());

        Self {
            source_text,
            span: relative_span,
            expected: SmallVec::new(),
            found: found.into(),
            line_offset: Some(line),
        }
    }
}
