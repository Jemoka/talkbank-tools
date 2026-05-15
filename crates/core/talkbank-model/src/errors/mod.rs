//! Error types for CHAT parsing and validation
//!
//! # Design Principles
//!
//! - Keep `ErrorSink` streaming; avoid collecting errors inside parsers
//! - Prefer `Result` for recoverable errors; avoid silent defaults
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Examples
//!
//! ```
//! use talkbank_model::{ErrorCollector, ErrorCode, ErrorSink, ParseError, Severity, Span};
//!
//! // Create a span and push an error into a streaming sink
//! let span = Span::new(0, 5);
//! let error = ParseError::at_span(ErrorCode::ParseFailed, Severity::Error, span, "bad token");
//! let sink = ErrorCollector::new();
//! sink.report(error);
//! assert_eq!(sink.len(), 1);
//! ```

/// Async-compatible channel-backed sink implementations.
#[cfg(feature = "async")]
pub mod async_channel_sink;
/// Builder pattern for constructing `ParseError` instances.
pub mod builder;
/// CHAT format text processing for display output.
pub mod chat_formatting;
/// CLAN-adjusted error location resolution (hidden header handling).
pub mod clan_location;
/// Error code definitions for CHAT parsing and validation.
pub mod codes;
/// In-memory error collectors and counters.
pub mod collectors;
/// Validation configuration for customizing error severity and filtering.
pub mod config;
/// Configurable error sink that applies validation configuration.
pub mod configurable_sink;
/// Rich error context for helpful error messages.
pub mod context;
/// Error enhancement utilities for adding line/column and source context.
pub mod enhance;
/// Core error sink trait plus lightweight forwarding implementations.
pub mod error_sink;
/// Byte-offset line index for O(log n) line/column lookups.
pub mod line_map;
/// Offset-adjusting error sink for wrapper technique offset adjustment.
pub mod offset_adjusting_sink;
/// Core diagnostic type: `ParseError` with source-backed context.
pub mod parse_error;
/// Collection type: `ParseErrors` and `ParseResult` type alias.
pub mod parse_errors;
/// Source location types: `SourceLocation`, `ErrorLabel`, `Severity`, `ErrorVec`.
pub mod source_location;
/// Span shifting trait for adjusting byte offsets in errors and AST nodes.
pub mod span_shift;
/// Error sink adapter that duplicates diagnostics to two downstream sinks.
pub mod tee_sink;

#[cfg(test)]
mod tests;

#[cfg(feature = "async")]
pub use async_channel_sink::AsyncChannelErrorSink;
pub use builder::{ParseErrorBuilder, ParseErrorBuilderError};
pub use clan_location::{ClanHiddenLineError, ClanLocation, resolve_clan_location};
pub use codes::ErrorCode;
pub use collectors::{ErrorCollector, ParseTracker};
pub use config::ValidationConfig;
pub use configurable_sink::ConfigurableErrorSink;
pub use context::ErrorContext;
pub use enhance::{enhance_errors_with_line_map, enhance_errors_with_source};
#[cfg(feature = "channels")]
pub use error_sink::ChannelErrorSink;
pub use error_sink::{ErrorSink, NullErrorSink};
pub use line_map::LineMap;
pub use offset_adjusting_sink::OffsetAdjustingErrorSink;
pub use parse_error::ParseError;
pub use parse_errors::{ParseErrors, ParseResult};
pub use source_location::{ErrorLabel, ErrorVec, Severity, SourceLocation, SourceLocationError};
pub use span_shift::SpanShift;
pub use tee_sink::TeeErrorSink;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Range;
pub use text_size::{TextRange, TextSize};

// =============================================================================
// Span - Compact source location backed by text-size::TextRange
// =============================================================================

/// Compact source span representing a byte range in source text.
///
/// This is a thin wrapper around [`text_size::TextRange`] that preserves
/// our existing API while gaining the type-safe offset arithmetic from
/// the `text-size` ecosystem (used by rust-analyzer, rowan, etc.).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    /// Start byte offset (0-indexed, inclusive)
    pub start: u32,
    /// End byte offset (0-indexed, exclusive)
    pub end: u32,
}

impl Span {
    /// Dummy span for programmatic construction (tests, builders).
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    /// Create a span from byte offsets.
    ///
    /// - `start`: inclusive start byte offset (0-indexed).
    /// - `end`: exclusive end byte offset (0-indexed).
    #[inline]
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Create a span from `usize` byte offsets, truncating to `u32`.
    ///
    /// Convenience constructor for call sites that work with `usize` lengths
    /// (e.g., `str::len()`). The offsets are cast with `as u32`, so values
    /// larger than `u32::MAX` will be silently truncated.
    #[inline]
    pub fn from_usize(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }

    /// Create a zero-width span at a single byte offset
    #[inline]
    pub fn at(offset: u32) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Check if this is a dummy span
    #[inline]
    pub fn is_dummy(&self) -> bool {
        self.start == 0 && self.end == 0
    }

    /// Get the length in bytes
    #[inline]
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Check if span is empty (zero-width)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Convert to `Range<usize>` for slicing.
    #[inline]
    pub fn to_range(&self) -> Range<usize> {
        (self.start as usize)..(self.end as usize)
    }

    /// Merge two spans into the smallest span that covers both.
    ///
    /// The result starts at `min(self.start, other.start)` and ends at
    /// `max(self.end, other.end)`. Useful for computing the span of a
    /// composite AST node from its children.
    #[inline]
    pub fn merge(&self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Convert to the underlying `text_size::TextRange`
    #[inline]
    pub fn to_text_range(&self) -> TextRange {
        TextRange::new(TextSize::new(self.start), TextSize::new(self.end))
    }

    /// Check if this span fully contains another span
    #[inline]
    pub fn contains_span(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Check if this span contains a byte offset
    #[inline]
    pub fn contains_offset(&self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }
}

impl From<Range<usize>> for Span {
    /// Convert a byte range into a `Span`.
    fn from(range: Range<usize>) -> Self {
        Span::from_usize(range.start, range.end)
    }
}

impl From<Span> for Range<usize> {
    /// Convert a `Span` into a range suitable for slicing.
    fn from(span: Span) -> Self {
        span.to_range()
    }
}

impl From<TextRange> for Span {
    /// Convert a `TextRange` into `Span` while preserving byte offsets.
    #[inline]
    fn from(range: TextRange) -> Self {
        Span {
            start: range.start().into(),
            end: range.end().into(),
        }
    }
}

impl From<Span> for TextRange {
    /// Convert `Span` into `text_size::TextRange`.
    #[inline]
    fn from(span: Span) -> Self {
        TextRange::new(TextSize::new(span.start), TextSize::new(span.end))
    }
}

// =============================================================================
// Spanned trait - uniform span access for all AST/model nodes
// =============================================================================

/// Trait for types that have a source span.
///
/// Implement this for every model type that carries source location information.
/// This provides a uniform way to extract spans from any node in the AST.
pub trait Spanned {
    /// Returns the source span of this node.
    fn span(&self) -> Span;
}
