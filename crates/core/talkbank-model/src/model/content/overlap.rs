//! Conversation Analysis overlap-point markers (`⌈⌉⌊⌋`).
//!
//! This module models boundary tokens that mark synchronized overlap regions in
//! speech, including optional numeric disambiguation for concurrent overlaps.
//!
//! # Terminology
//!
//! - **Overlap points** (also called "CA overlaps"): The ⌈⌉⌊⌋ brackets marking speech boundaries
//! - **Overlap markers**: The [>] and [<] postcodes indicating speaker overlap order
//!
//! # Format
//!
//! Overlap points use paired Unicode brackets:
//! - **Top brackets**: ⌈ (begin) and ⌉ (end) - mark one speaker's overlap
//! - **Bottom brackets**: ⌊ (begin) and ⌋ (end) - mark the other speaker's overlap
//!
//! # Simple Overlap Example
//!
//! ```text
//! *MOT: I think ⌈ we should go ⌉ .
//! *CHI:         ⌊ can I come   ⌋ ?
//! ```
//!
//! In this example, the child begins speaking while the mother is still talking.
//! The overlapping portions are "we should go" and "can I come".
//!
//! # Indexed Overlaps
//!
//! For multiple simultaneous overlaps, add single-digit indices (2-9):
//!
//! ```text
//! *MOT: I think ⌈2 we should ⌉2 go ⌈3 now ⌉3 .
//! *CHI:         ⌊2 can I      ⌋2    ⌊3 yes ⌋3 ?
//! *FAT:                           ⌊3 okay ⌋3 .
//! ```
//!
//! Here, two overlaps occur:
//! 1. MOT's "we should" overlaps with CHI's "can I"
//! 2. MOT's "now" overlaps with both CHI's "yes" and FAT's "okay"
//!
//! # Three-Way Overlap Example
//!
//! ```text
//! *MOT: we need to ⌈ go home ⌉ now .
//! *CHI:            ⌊ I'm tired ⌋ .
//! *FAT:            ⌊ me too ⌋ .
//! ```
//!
//! Both CHI and FAT use bottom brackets because they both overlap with MOT.
//!
//! # CHAT Manual Reference
//!
//! - [CA Overlaps (Overlap Points)](https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps)
//! - [Top Begin Overlap Delimiter](https://talkbank.org/0info/manuals/CHAT.html#TopBeginOverlap_Delimiter)
//! - [Bottom Begin Overlap Delimiter](https://talkbank.org/0info/manuals/CHAT.html#BottomBeginOverlap_Delimiter)
//! - [Overlap Markers [>]/[<]](https://talkbank.org/0info/manuals/CHAT.html#OverlapPrecedes_Scope)

use super::WriteChat;
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Kinds of CA overlap-point boundary markers.
///
/// Top and bottom forms are used in paired lines to delimit concurrent speech.
///
/// # Terminology Note
///
/// This type represents **overlap points** (also called "CA overlaps"), NOT **overlap markers**.
/// - **Overlap points**: ⌈⌉⌊⌋ brackets marking speech boundaries (this type)
/// - **Overlap markers**: `[>]` and `[<]` postcodes indicating speaker order (different type)
///
/// # Point Types
///
/// - **TopOverlapBegin** (⌈): Start of top speaker's overlap
/// - **TopOverlapEnd** (⌉): End of top speaker's overlap
/// - **BottomOverlapBegin** (⌊): Start of bottom speaker's overlap
/// - **BottomOverlapEnd** (⌋): End of bottom speaker's overlap
///
/// # Simple Example
///
/// ```
/// use talkbank_model::model::{OverlapIndex, OverlapPoint, OverlapPointKind};
///
/// // Mother's overlap points (top)
/// let begin = OverlapPoint::new(OverlapPointKind::TopOverlapBegin, None);
/// let end = OverlapPoint::new(OverlapPointKind::TopOverlapEnd, None);
/// assert_eq!(begin.to_string(), "⌈");
/// assert_eq!(end.to_string(), "⌉");
///
/// // Child's overlap points (bottom)
/// let begin = OverlapPoint::new(OverlapPointKind::BottomOverlapBegin, None);
/// let end = OverlapPoint::new(OverlapPointKind::BottomOverlapEnd, None);
/// assert_eq!(begin.to_string(), "⌊");
/// assert_eq!(end.to_string(), "⌋");
/// ```
///
/// # Indexed Example
///
/// ```
/// use talkbank_model::model::{OverlapIndex, OverlapPoint, OverlapPointKind};
///
/// // Multiple overlaps require single-digit indices (2-9)
/// let overlap1_begin = OverlapPoint::new(OverlapPointKind::TopOverlapBegin, Some(OverlapIndex::new(2)));
/// let overlap2_begin = OverlapPoint::new(OverlapPointKind::TopOverlapBegin, Some(OverlapIndex::new(3)));
/// assert_eq!(overlap1_begin.to_string(), "⌈2");
/// assert_eq!(overlap2_begin.to_string(), "⌈3");
/// ```
///
/// # CHAT Transcript Example
///
/// ```text
/// *MOT: look at ⌈ the dog ⌉ over there .
/// *CHI:         ⌊ woof    ⌋ !
/// ```
///
/// # CHAT Manual Reference
///
/// - [CA Overlaps](https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps)
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(rename_all = "snake_case")]
pub enum OverlapPointKind {
    /// `⌈` top begin marker.
    TopOverlapBegin,
    /// `⌉` top end marker.
    TopOverlapEnd,
    /// `⌊` bottom begin marker.
    BottomOverlapBegin,
    /// `⌋` bottom end marker.
    BottomOverlapEnd,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    SemanticEq,
    SpanShift,
)]
#[serde(transparent)]
/// Numeric disambiguator for concurrent overlap regions.
///
/// Reference:
/// - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
pub struct OverlapIndex(pub u32);

impl OverlapIndex {
    /// Wrap raw index value; range checks are performed by validation.
    ///
    /// Construction is intentionally permissive so parser paths can preserve
    /// source values and report context-rich diagnostics later.
    pub fn new(index: u32) -> Self {
        Self(index)
    }
}

impl std::fmt::Display for OverlapIndex {
    /// Formats overlap index as a digit for CHAT serialization.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
/// Single overlap-point token with optional disambiguation index.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
/// - <https://talkbank.org/0info/manuals/CHAT.html#TopBeginOverlap_Delimiter>
/// - <https://talkbank.org/0info/manuals/CHAT.html#BottomBeginOverlap_Delimiter>
pub struct OverlapPoint {
    /// Marker shape (`top/bottom` x `begin/end`).
    #[serde(rename = "marker")]
    pub kind: OverlapPointKind,
    /// Optional overlap index for concurrent overlap regions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<OverlapIndex>,
    /// Source location metadata for diagnostics.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub span: Option<Span>,
}

impl OverlapPoint {
    /// Build an overlap-point token with no span metadata.
    ///
    /// This keeps direct model construction concise; parser-produced values can
    /// attach real spans via [`Self::with_span`].
    pub fn new(kind: OverlapPointKind, index: Option<OverlapIndex>) -> Self {
        Self {
            kind,
            index,
            span: None,
        }
    }

    /// Attach source span metadata for diagnostics.
    ///
    /// Span data does not alter serialized overlap marker text.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl Validate for OverlapIndex {
    /// Enforces CHAT overlap-index range (single digit `2` through `9`).
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        if !matches!(self.0, 2..=9) {
            let index_str = self.0.to_string();
            let span = match context.field_span {
                Some(span) => span,
                None => crate::Span::from_usize(0, index_str.len()),
            };
            let location = match context.field_span {
                Some(span) => SourceLocation::new(span),
                None => SourceLocation::at_offset(0),
            };
            let source_text = match context.field_text.clone() {
                Some(text) => text,
                None => index_str.clone(),
            };
            // DEFAULT: Missing label falls back to "overlap_index" for error messaging.
            let label = context.field_label.unwrap_or("overlap_index");
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidOverlapIndex,
                    Severity::Error,
                    location,
                    ErrorContext::new(source_text, span, label),
                    format!("Overlap index {} is invalid", self.0),
                )
                .with_suggestion("Overlap indices must be a single digit from 2 to 9"),
            );
        }
    }
}

impl Validate for OverlapPoint {
    /// Validates attached overlap index when present.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        if let Some(index) = self.index {
            index.validate(context, errors);
        }
    }
}

impl OverlapPoint {
    /// Returns `true` for begin markers (`⌈`/`⌊`).
    ///
    /// Opening overlap markers should NOT have a space after them in serialization.
    /// Example: `⌈very⌉` not `⌈ very⌉`
    pub fn is_opening(&self) -> bool {
        matches!(
            self.kind,
            OverlapPointKind::TopOverlapBegin | OverlapPointKind::BottomOverlapBegin
        )
    }

    /// Returns `true` for end markers (`⌉`/`⌋`).
    ///
    /// Closing overlap markers should NOT have a space before them in serialization.
    /// Example: `⌈very⌉` not `⌈very ⌉`
    pub fn is_closing(&self) -> bool {
        matches!(
            self.kind,
            OverlapPointKind::TopOverlapEnd | OverlapPointKind::BottomOverlapEnd
        )
    }

    /// Returns delimiter glyph without numeric suffix.
    fn base_char(&self) -> &'static str {
        match self.kind {
            OverlapPointKind::TopOverlapBegin => "\u{2308}", // ⌈
            OverlapPointKind::TopOverlapEnd => "\u{2309}",   // ⌉
            OverlapPointKind::BottomOverlapBegin => "\u{230A}", // ⌊
            OverlapPointKind::BottomOverlapEnd => "\u{230B}", // ⌋
        }
    }

    /// Returns optional disambiguation index.
    pub fn index(&self) -> Option<OverlapIndex> {
        self.index
    }
}

impl WriteChat for OverlapPoint {
    /// Writes overlap point in CHAT text form (delimiter + optional index).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.base_char())?;
        if let Some(idx) = self.index {
            write!(w, "{}", idx)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for OverlapPoint {
    /// Formats overlap marker exactly as CHAT surface text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
