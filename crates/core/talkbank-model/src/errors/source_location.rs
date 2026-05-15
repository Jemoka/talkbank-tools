//! Source location types and span-to-location conversions.
//!
//! Provides [`SourceLocation`] for mapping byte offsets to line/column positions,
//! [`ErrorLabel`] for secondary diagnostic spans, [`Severity`] for error classification,
//! and the [`ErrorVec`] type alias for small-vec-optimized error collections.

use super::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt;

use super::parse_error::ParseError;

// =============================================================================
// Error Vector Optimization
// =============================================================================

/// A secondary label attached to a diagnostic, pointing to a related source span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorLabel {
    /// Byte range of the labeled span.
    #[serde(flatten)]
    pub span: Span,
    /// Descriptive message for this label.
    pub message: String,
}

impl ErrorLabel {
    /// Create a new error label pointing to a related source span.
    ///
    /// # Parameters
    ///
    /// - `span`: Byte range of the related source location.
    /// - `message`: Descriptive text rendered alongside the underlined span
    ///   (e.g., "first declared here", "expected this type").
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

/// Optimized error vector that stores up to 2 errors inline before heap allocation.
pub type ErrorVec = SmallVec<[ParseError; 2]>;

/// Severity class for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Invalid CHAT syntax - must be fixed
    Error,
    /// Valid but deprecated/stylistic issue - should be fixed
    Warning,
}

impl fmt::Display for Severity {
    /// Render severity as the lowercase label expected by CLI/UI reporters.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

/// Source code location in error diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceLocation {
    /// The underlying span
    #[serde(flatten)]
    pub span: Span,
    /// Line number in the source file (1-indexed, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Column number in the line (1-indexed, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

/// Errors for source-based location construction.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error,
)]
pub enum SourceLocationError {
    /// A byte offset exceeds the source text length.
    #[error("offset {offset} is out of bounds for source length {source_len}")]
    OffsetOutOfBounds {
        /// The out-of-bounds byte offset.
        offset: usize,
        /// Length of the source text.
        source_len: usize,
    },
    /// A byte span is invalid for the given source text.
    #[error("span {start}..{end} is invalid for source length {source_len}")]
    SpanOutOfBounds {
        /// Start byte offset of the span.
        start: usize,
        /// End byte offset of the span.
        end: usize,
        /// Length of the source text.
        source_len: usize,
    },
}

impl SourceLocation {
    /// Create a source location from a byte span, without line/column information.
    ///
    /// Line and column fields are set to `None`. Use [`with_position`](Self::with_position)
    /// or [`from_offsets_with_position`](Self::from_offsets_with_position) to add them.
    pub fn new(span: Span) -> Self {
        Self {
            span,
            line: None,
            column: None,
        }
    }

    /// Create a source location from start and end byte offsets, without line/column.
    ///
    /// Equivalent to `SourceLocation::new(Span::from_usize(start, end))`.
    pub fn from_offsets(start: usize, end: usize) -> Self {
        Self {
            span: Span::from_usize(start, end),
            line: None,
            column: None,
        }
    }

    /// Create a source location from byte offsets with pre-computed line/column.
    ///
    /// # Parameters
    ///
    /// - `start`, `end`: Byte offsets (0-indexed, end is exclusive).
    /// - `line`, `column`: 1-indexed line and column numbers for display.
    pub fn from_offsets_with_position(
        start: usize,
        end: usize,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            span: Span::from_usize(start, end),
            line: Some(line),
            column: Some(column),
        }
    }

    /// Create from byte offset in source text, calculating line/column.
    ///
    /// Returns an error when `offset` is out of bounds for `source`.
    pub fn from_offset_in_source(offset: usize, source: &str) -> Result<Self, SourceLocationError> {
        if offset > source.len() {
            return Err(SourceLocationError::OffsetOutOfBounds {
                offset,
                source_len: source.len(),
            });
        }
        let (line, column) = Self::calculate_line_column(offset, source);
        Ok(Self {
            span: Span::from_usize(offset, offset),
            line: Some(line),
            column: Some(column),
        })
    }

    /// Create from byte offsets in source text, calculating line/column for start position.
    ///
    /// Returns an error when the span is invalid for `source`.
    pub fn from_offsets_in_source(
        start: usize,
        end: usize,
        source: &str,
    ) -> Result<Self, SourceLocationError> {
        if start > end || end > source.len() {
            return Err(SourceLocationError::SpanOutOfBounds {
                start,
                end,
                source_len: source.len(),
            });
        }
        let (line, column) = Self::calculate_line_column(start, source);
        Ok(Self {
            span: Span::from_usize(start, end),
            line: Some(line),
            column: Some(column),
        })
    }

    /// Calculate line and column (1-indexed) from byte offset in source.
    ///
    /// Uses [`LineMap`](crate::LineMap) for O(log n) lookup. The LineMap is cached
    /// in thread-local storage keyed by `(source_ptr, source_len)`, so repeated
    /// calls for the same source (e.g. multiple errors in one file) reuse the
    /// same LineMap without rebuilding.
    pub fn calculate_line_column(offset: usize, source: &str) -> (usize, usize) {
        use std::cell::RefCell;

        thread_local! {
            static LINE_MAP_CACHE: RefCell<Option<(usize, usize, crate::LineMap)>> = const { RefCell::new(None) };
        }

        LINE_MAP_CACHE.with_borrow_mut(|cache| {
            let ptr = source.as_ptr() as usize;
            let len = source.len();
            let cached = cache.get_or_insert_with(|| (ptr, len, crate::LineMap::new(source)));
            if cached.0 != ptr || cached.1 != len {
                *cached = (ptr, len, crate::LineMap::new(source));
            }
            let (line_0, col_0) = cached.2.line_col_of(offset as u32);
            (line_0 + 1, col_0 + 1)
        })
    }

    /// Create a zero-width location at a single byte offset (no line/column).
    ///
    /// The resulting span has `start == end == offset`, useful for insertion
    /// diagnostics that point *between* characters rather than *at* a range.
    pub fn at_offset(offset: usize) -> Self {
        Self {
            span: Span::from_usize(offset, offset),
            line: None,
            column: None,
        }
    }

    /// Create a source location from a `Range<usize>`, without line/column.
    pub fn from_range(range: std::ops::Range<usize>) -> Self {
        Self {
            span: Span::from(range),
            line: None,
            column: None,
        }
    }

    /// Add 1-indexed line/column information to this location.
    ///
    /// Returns `self` for method chaining. Typically called after
    /// [`new`](Self::new) or [`from_offsets`](Self::from_offsets) when
    /// line/column data becomes available later (e.g., from a [`LineMap`](crate::LineMap)).
    pub fn with_position(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

impl From<Span> for SourceLocation {
    /// Build a location from span-only information.
    fn from(span: Span) -> Self {
        Self {
            span,
            line: None,
            column: None,
        }
    }
}

impl From<Span> for miette::SourceSpan {
    /// Convert absolute byte-span data into `miette` source-span format.
    fn from(span: Span) -> Self {
        (span.start as usize, span.len() as usize).into()
    }
}

impl From<Severity> for miette::Severity {
    /// Map parser severity to `miette` severity.
    fn from(s: Severity) -> Self {
        match s {
            Severity::Error => miette::Severity::Error,
            Severity::Warning => miette::Severity::Warning,
        }
    }
}
