//! End marker for scoped nonvocal events (`&}n=label`).
//!
//! This marker closes the currently open nonvocal scope for the same label.
//! Validators use it together with [`NonvocalBegin`](super::NonvocalBegin) to
//! enforce pairing and nesting correctness.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent>

use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::WriteChat;
use super::NonvocalLabel;

/// End boundary for a scoped nonvocal event.
///
/// A valid scope pairs this marker with an earlier [`super::NonvocalBegin`]
/// using the same label.
///
/// # CHAT Format
///
/// ```text
/// &}n=LABEL
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: &{n=crying I want mommy .
/// *CHI: please &}n=crying .
/// ```
///
/// The `&}n=crying` marker closes the crying event that began with `&{n=crying`.
///
/// # Validation
///
/// The source span enables validation that:
/// - Each end marker has a matching begin marker
/// - Labels match between paired begin/end markers
/// - Scopes are properly nested (no crossed scopes)
///
/// # References
///
/// - [Long Nonverbal Event](https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent)
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct NonvocalEnd {
    /// Label used to pair begin/end boundaries.
    pub label: NonvocalLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl PartialEq for NonvocalEnd {
    /// Equality is label-based; source span is intentionally ignored.
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl Eq for NonvocalEnd {}

impl NonvocalEnd {
    /// Build an end marker with dummy span metadata.
    ///
    /// Parser code should attach concrete spans via [`Self::with_span`] so
    /// unmatched-end diagnostics can identify the original token precisely.
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

impl WriteChat for NonvocalEnd {
    /// Writes this nonvocal end marker in CHAT format.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "&}}n={}", self.label)
    }
}

impl std::fmt::Display for NonvocalEnd {
    /// Formats this end marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
