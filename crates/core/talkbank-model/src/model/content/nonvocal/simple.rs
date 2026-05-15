//! Point nonvocal marker (`&{n=label}`) for instantaneous events.
//!
//! Unlike begin/end markers, this form encodes a self-contained event token
//! and therefore does not participate in cross-token scope pairing checks.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent>

use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::WriteChat;
use super::NonvocalLabel;

/// Point nonvocal event marker (`&{n=LABEL}`).
///
/// Unlike scoped begin/end markers, this form represents a single localized
/// nonvocal event.
///
/// # CHAT Format
///
/// ```text
/// &{n=LABEL}
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: I want &{n=cough} cookie .
/// *MOT: look at this &{n=laugh} .
/// *CHI: &{n=sneeze} excuse me .
/// ```
///
/// # Difference from Scoped Events
///
/// Simple markers (`&{n=laugh}`) are used for instantaneous or brief events,
/// while scoped markers (`&{n=crying ... &}n=crying`) span multiple words or
/// content elements.
///
/// # References
///
/// - [Long Nonverbal Event](https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent)
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct NonvocalSimple {
    /// Event label payload.
    pub label: NonvocalLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl PartialEq for NonvocalSimple {
    /// Equality is label-based; source span is intentionally ignored.
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl Eq for NonvocalSimple {}

impl NonvocalSimple {
    /// Build a simple nonvocal marker with dummy span metadata.
    ///
    /// Parser code should attach concrete spans via [`Self::with_span`] so
    /// downstream diagnostics can localize event tokens in source text.
    pub fn new(label: impl Into<NonvocalLabel>) -> Self {
        Self {
            label: label.into(),
            span: Span::DUMMY,
        }
    }

    /// Attach source span metadata used for diagnostics.
    ///
    /// Span values are intentionally excluded from semantic equality.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for NonvocalSimple {
    /// Writes this nonvocal simple marker in CHAT format.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "&{{n={}}}", self.label)
    }
}

impl std::fmt::Display for NonvocalSimple {
    /// Formats this simple nonvocal marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
