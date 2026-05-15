//! Long-event boundary markers (`&{l=...` / `&}l=...`).
//!
//! These markers encode scoped phenomena that may span multiple words or even
//! multiple utterances.
//!
//! # CHAT Format References
//!
//! - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)

use super::WriteChat;
use crate::Span;
use crate::string_newtype;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

string_newtype!(
    /// Long-event label text written after `l=`.
    ///
    /// Modeled as an unconstrained newtype because corpora commonly use
    /// project-specific labels.
    ///
    /// # CHAT Format Examples
    ///
    /// ```text
    /// &{l=overlap        Begin overlapping speech
    /// &}l=overlap        End overlapping speech
    /// &{l=singing        Begin singing
    /// &}l=singing        End singing
    /// &{l=whisper        Begin whispered speech
    /// &}l=whisper        End whispered speech
    /// ```
    ///
    /// # Common Labels
    ///
    /// - **Overlapping speech**: overlap, simultaneous
    /// - **Prosodic features**: singing, chanting, whisper, shout, emphasis
    /// - **Code-switching**: code-switch, mixed-language
    /// - **Quotation**: quote, reported-speech
    /// - **Other discourse**: sarcasm, irony, imitation
    ///
    /// # Usage
    ///
    /// Long events mark spans where the marked feature applies continuously across
    /// multiple words. The begin marker (`&{l=LABEL`) starts the scope, and the end
    /// marker (`&}l=LABEL`) closes it. All words between the markers are affected
    /// by the feature.
    ///
    /// # References
    ///
    /// - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)
    #[serde(transparent)]
    pub struct LongFeatureLabel;
);

/// Begin boundary for a long-event scope.
///
/// A valid scope pairs this marker with a later [`LongFeatureEnd`] carrying the
/// same label.
///
/// # CHAT Format
///
/// ```text
/// &{l=LABEL
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: &{l=singing happy birthday to you &}l=singing .
/// *MOT: what did you &{l=whisper say to him &}l=whisper ?
/// *CHI: &{l=overlap I want &}l=overlap one .
/// *FAT: &{l=overlap no you can't &}l=overlap .
/// ```
///
/// # Overlapping Speech
///
/// For overlapping speech between speakers, use matching `&{l=overlap` and
/// `&}l=overlap` markers in both speakers' utterances to show where the
/// simultaneous speech occurs.
///
/// # Validation
///
/// The source span enables validation that begin markers are properly paired
/// with matching end markers. Unpaired markers generate validation errors.
///
/// # References
///
/// - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)
/// - [Overlaps](https://talkbank.org/0info/manuals/CHAT.html#Overlaps)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct LongFeatureBegin {
    /// Label used to pair begin/end boundaries.
    pub label: LongFeatureLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl LongFeatureBegin {
    /// Build a begin marker with dummy span metadata.
    ///
    /// Parser/test code can use this constructor before source offsets are
    /// attached; semantic behavior depends only on the label payload.
    pub fn new(label: impl Into<LongFeatureLabel>) -> Self {
        Self {
            label: label.into(),
            span: Span::DUMMY,
        }
    }

    /// Attach source span metadata used by diagnostics.
    ///
    /// Span values do not affect begin/end pairing semantics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for LongFeatureBegin {
    /// Writes this begin marker in CHAT long-event syntax (`&{l=...`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "&{{l={}", self.label)
    }
}

impl std::fmt::Display for LongFeatureBegin {
    /// Formats this begin marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

/// End boundary for a long-event scope.
///
/// A valid scope pairs this marker with an earlier [`LongFeatureBegin`] carrying
/// the same label.
///
/// # CHAT Format
///
/// ```text
/// &}l=LABEL
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: &{l=singing happy birthday to you &}l=singing .
/// *MOT: what did you &{l=whisper say to him &}l=whisper ?
/// ```
///
/// The end marker closes the scope opened by the matching begin marker.
///
/// # Cross-Utterance Scopes
///
/// Long event scopes can span multiple utterances:
///
/// ```text
/// *CHI: &{l=singing happy birthday to you .
/// *CHI: happy birthday dear mommy &}l=singing .
/// ```
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
/// - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct LongFeatureEnd {
    /// Label used to pair begin/end boundaries.
    pub label: LongFeatureLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl LongFeatureEnd {
    /// Build an end marker with dummy span metadata.
    ///
    /// Parser/test code can use this constructor before source offsets are
    /// attached; semantic behavior depends only on the label payload.
    pub fn new(label: impl Into<LongFeatureLabel>) -> Self {
        Self {
            label: label.into(),
            span: Span::DUMMY,
        }
    }

    /// Attach source span metadata used by diagnostics.
    ///
    /// Span values do not affect begin/end pairing semantics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}

impl WriteChat for LongFeatureEnd {
    /// Writes this end marker in CHAT long-event syntax (`&}l=...`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "&}}l={}", self.label)
    }
}

impl std::fmt::Display for LongFeatureEnd {
    /// Formats this end marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
