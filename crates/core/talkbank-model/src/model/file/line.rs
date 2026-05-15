//! Line representation for CHAT transcripts.
//!
//! `Line` is the file-order unit used by [`super::ChatFile`]. Each parsed line
//! is either a header (`@...`) or an utterance (`*SPEAKER: ...` with attached
//! dependent tiers).
//!
//! # Why `Line` exists
//!
//! CHAT files interleave headers and utterances:
//!
//! ```text
//! @UTF8
//! @Begin
//! @Languages: eng
//! *CHI: hello .
//! @Comment: This is between utterances
//! *MOT: hi there .
//! @Comment: Another comment
//! *CHI: bye .
//! @End
//! ```
//!
//! Keeping headers and utterances in separate collections loses this ordering
//! and breaks roundtrip fidelity for mid-file headers (`@Comment`, gems, etc.).
//! `Line` preserves exact source order.

use super::Utterance;
use crate::Span;
use crate::{Header, WriteChat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// One line-level unit in a CHAT transcript.
///
/// A value is either:
/// - `Header`: metadata beginning with `@`
/// - `Utterance`: speaker turn (`*...`) with its dependent tiers
///
/// The enum is intentionally minimal: it captures file ordering and delegates
/// line-type-specific behavior to `Header`/`Utterance`.
///
/// # CHAT Format Examples
///
/// A typical CHAT file interleaves headers and utterances:
///
/// ```text
/// @UTF8
/// @Begin
/// @Languages: eng
/// @Participants: CHI Child, MOT Mother
/// @ID: eng|corpus|CHI|2;06.00|male|||Child|||
/// *CHI: I want cookie .
/// %mor: pro:sub|I v|want n|cookie .
/// @Comment: Child reaching for cookie jar
/// *MOT: you can have one .
/// %mor: pro:per|you mod|can v|have det:num|one .
/// @End
/// ```
///
/// Each line is parsed as a `Line::Header` or `Line::Utterance`:
///
/// ```
/// use talkbank_model::model::{Line, Header, Utterance, MainTier, BulletContent};
///
/// let lines = vec![
///     Line::header(Header::Utf8),
///     Line::header(Header::Begin),
///     // Languages header
///     // Participants header
///     // Utterance: *CHI: I want cookie .
///     // Dependent tier: %mor
///     Line::header(Header::Comment { content: BulletContent::from_text("Between utterances") }),
///     // Utterance: *MOT: you can have one .
///     // Dependent tier: %mor
///     Line::header(Header::End),
/// ];
/// ```
///
/// # Ordering Guarantee
///
/// Comments and other headers can appear between utterances; preserving that
/// order is required for semantic roundtrips. Without `Line`, this structure
/// is lost:
///
/// ```text
/// *CHI: hello .
/// @Comment: First greeting
/// *MOT: hi there .
/// @Comment: Mother responds
/// *CHI: bye .
/// ```
///
/// The `Line` enum preserves that `@Comment` appears after specific utterances.
///
/// # References
///
/// - [File Format](https://talkbank.org/0info/manuals/CHAT.html#File_Format)
/// - [Main Line](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
/// - [File Headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "line_type", rename_all = "lowercase")]
pub enum Line {
    /// Header line (starts with `@`).
    Header {
        /// Parsed header payload.
        header: Box<Header>,

        /// Source span for diagnostics (not serialized).
        #[serde(skip)]
        #[schemars(skip)]
        span: Span,
    },

    /// Utterance line (main tier + dependent tiers).
    ///
    /// Span is derived from `Utterance.main.span`.
    Utterance(Box<Utterance>),
}

impl Line {
    /// Build a header line with dummy span metadata.
    ///
    /// Useful for fixtures and transformed model values where source offsets
    /// are not retained.
    pub fn header(header: Header) -> Self {
        Line::Header {
            header: Box::new(header),
            span: Span::DUMMY,
        }
    }

    /// Build a header line with explicit source span metadata.
    ///
    /// Parser-produced headers should prefer this to keep diagnostics precise.
    pub fn header_with_span(header: Header, span: Span) -> Self {
        Line::Header {
            header: Box::new(header),
            span,
        }
    }

    /// Wrap an utterance payload as a line variant.
    ///
    /// The utterance itself carries its own span via `main.span`.
    pub fn utterance(utterance: Utterance) -> Self {
        Line::Utterance(Box::new(utterance))
    }

    /// Returns `true` when this value is `Line::Header`.
    pub fn is_header(&self) -> bool {
        matches!(self, Line::Header { .. })
    }

    /// Returns `true` when this value is `Line::Utterance`.
    pub fn is_utterance(&self) -> bool {
        matches!(self, Line::Utterance(_))
    }

    /// Borrow header payload if this line is a header.
    pub fn as_header(&self) -> Option<&Header> {
        match self {
            Line::Header { header, .. } => Some(header),
            Line::Utterance(_) => None,
        }
    }

    /// Borrow utterance payload if this line is an utterance.
    pub fn as_utterance(&self) -> Option<&Utterance> {
        match self {
            Line::Header { .. } => None,
            Line::Utterance(u) => Some(u),
        }
    }

    /// Return this line's source span.
    pub fn span(&self) -> Span {
        match self {
            Line::Header { span, .. } => *span,
            Line::Utterance(u) => u.main.span,
        }
    }
}

impl WriteChat for Line {
    /// Serialize one line as CHAT text.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            Line::Header { header, .. } => header.write_chat(w),
            Line::Utterance(u) => u.write_chat(w),
        }
    }
}
