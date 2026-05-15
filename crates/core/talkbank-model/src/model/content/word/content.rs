//! Internal word-content tokens used by the CHAT word model.
//!
//! These types describe what can appear *inside* one word token after lexical
//! parsing (text segments, shortenings, prosodic markers, CA markers, and
//! control markers).
//!
//! # Word Content Types
//!
//! - **Text**: Plain text segments within words
//! - **Shortening**: Omitted sounds in parentheses (e.g., `(be)cause`)
//! - **StressMarker**: Primary/secondary stress markers (ˈ, ˌ)
//! - **Lengthening**: Syllable lengthening marker (:)
//! - **SyllablePause**: Pause between syllables (^)
//! - **OverlapPoint**: CA overlap point markers within words (⌈⌉⌊⌋ with optional indices)
//! - **CAElement**: Individual CA prosodic markers (pitch, stress, etc.)
//! - **CADelimiter**: Paired CA prosodic markers (faster, softer, etc.)
//! - **UnderlineBegin/End**: Control characters for underlined text
//!
//! # CHAT Format Examples
//!
//! **Shortenings (omitted sounds):**
//! ```text
//! *CHI: (be)cause .                      # "because" with "be" omitted
//! *CHI: (a)bout .                        # "about" with "a" omitted
//! *MOT: gonna (go)ing .                  # "going" with "go" omitted
//! ```
//!
//! **Overlap Points (within words):**
//! ```text
//! *CHI: I w⌈ant it .                     # Overlap begins during "want"
//! *MOT: no you⌉ can't .                  # Overlap ends during "you"
//! ```
//!
//! **Combined Elements:**
//! ```text
//! *CHI: ↑really ?                        # Pitch rise
//! *MOT: (be)∆cause∆ .                    # Shortening + faster speech
//! *CHI: ˈvery nice .                     # Primary stress on "very"
//! ```
//!
//! # References
//!
//! - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
//! - [Overlap](https://talkbank.org/0info/manuals/CHAT.html#Overlap)
//! - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)

use crate::model::{NonEmptyString, OverlapPoint, WriteChat};
use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorSink};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// One token in a word's internal content stream.
///
/// A complete [`crate::model::Word`] is an ordered sequence of these items plus
/// optional outer markers (category, form, language, POS).
///
/// # Variants
///
/// - **Text**: Plain text segment (e.g., `hello`, `want`)
/// - **Shortening**: Omitted sound in parentheses (e.g., `(be)` in `(be)cause`)
/// - **OverlapPoint**: CA overlap point marker within word (`⌈`, `⌉`, `⌊`, `⌋` with optional indices)
/// - **CAElement**: Individual CA prosodic marker (e.g., `↑`, `ˈ`, `∙`)
/// - **CADelimiter**: Paired CA prosodic marker (e.g., `∆`, `°`, `∬`)
/// - **StressMarker**: Primary/secondary stress markers (ˈ, ˌ)
/// - **Lengthening**: Syllable lengthening marker (:)
/// - **SyllablePause**: Pause between syllables (^)
/// - **UnderlineBegin/End**: Control characters for underlined text
///
/// # CHAT Format Examples
///
/// ```text
/// (be)cause          # Shortening + Text
/// ↑hello             # CAElement + Text
/// wo⌈rd              # Text + OverlapPoint (overlap begins mid-word)
/// ∆fast∆             # CADelimiter + Text + CADelimiter
/// ˈvery              # StressMarker + Text
/// ```
///
/// # References
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
/// - [Overlap](https://talkbank.org/0info/manuals/CHAT.html#Overlap)
/// - [Annotations](https://talkbank.org/0info/manuals/CHAT.html#Annotations)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", content = "content", rename_all = "lowercase")]
pub enum WordContent {
    /// Plain text segment
    Text(WordText),
    /// (lo) - shortening (omitted sound)
    Shortening(WordShortening),
    /// Overlap point within word
    OverlapPoint(OverlapPoint),
    /// CA element (individual prosody marker like ↑, ↓, ≠, ∾)
    #[serde(rename = "ca_element")]
    CAElement(super::ca::CAElement),
    /// CA delimiter (paired prosody marker like ∆, ∇, °, ▁)
    #[serde(rename = "ca_delimiter")]
    CADelimiter(super::ca::CADelimiter),
    /// Stress marker (ˈ, ˌ) - NOT a CA element
    #[serde(rename = "stress_marker")]
    StressMarker(WordStressMarker),
    /// Lengthening marker (:) - NOT a CA element
    #[serde(rename = "lengthening")]
    Lengthening(WordLengthening),
    /// Pause between syllables (^) - NOT a CA element
    #[serde(rename = "syllable_pause")]
    SyllablePause(WordSyllablePause),
    /// Underline begin marker (\u0002\u0001)
    UnderlineBegin(WordUnderlineBegin),
    /// Underline end marker (\u0002\u0002)
    UnderlineEnd(WordUnderlineEnd),
    /// Compound marker (+) - NOT part of phonetic content
    ///
    /// Marks word-internal compound boundaries (e.g., `ice+cream`, `wai4+yu3`).
    /// This is metadata about word structure and is NOT included in `cleaned_text`.
    #[serde(rename = "compound_marker")]
    CompoundMarker(WordCompoundMarker),
    /// Clitic boundary (~) - marks morphological clitic attachment
    ///
    /// Common in Arabic (`le~ha`, `bba~hum`) and other languages.
    /// This is structural metadata and is NOT included in `cleaned_text`.
    #[serde(rename = "clitic_boundary")]
    CliticBoundary(WordCliticBoundary),
}

impl WriteChat for WordContent {
    /// Serializes a word-internal element using its CHAT surface form.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            WordContent::Text(text) => w.write_str(text),
            WordContent::Shortening(text) => {
                w.write_char('(')?;
                w.write_str(text)?;
                w.write_char(')')
            }
            WordContent::OverlapPoint(point) => point.write_chat(w),
            WordContent::CAElement(ca) => ca.write_chat(w),
            WordContent::CADelimiter(ca) => ca.write_chat(w),
            WordContent::StressMarker(marker) => marker.write_chat(w),
            WordContent::Lengthening(marker) => marker.write_chat(w),
            WordContent::SyllablePause(marker) => marker.write_chat(w),
            WordContent::UnderlineBegin(_) => {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0001}')
            }
            WordContent::UnderlineEnd(_) => {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0002}')
            }
            WordContent::CompoundMarker(_) => w.write_char('+'),
            WordContent::CliticBoundary(marker) => marker.write_chat(w),
        }
    }
}

impl Validate for WordContent {
    /// Delegates validation to the concrete inner marker/token type.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        match self {
            WordContent::Text(text) => text.validate(context, errors),
            WordContent::Shortening(text) => text.validate(context, errors),
            WordContent::OverlapPoint(point) => point.validate(context, errors),
            WordContent::CAElement(element) => element.validate(context, errors),
            WordContent::CADelimiter(delimiter) => delimiter.validate(context, errors),
            WordContent::StressMarker(marker) => marker.validate(context, errors),
            WordContent::Lengthening(marker) => marker.validate(context, errors),
            WordContent::SyllablePause(marker) => marker.validate(context, errors),
            WordContent::UnderlineBegin(marker) => marker.validate(context, errors),
            WordContent::UnderlineEnd(marker) => marker.validate(context, errors),
            WordContent::CompoundMarker(marker) => marker.validate(context, errors),
            WordContent::CliticBoundary(marker) => marker.validate(context, errors),
        }
    }
}

/// A plain text segment within a word.
///
/// Wraps a [`NonEmptyString`] guaranteeing the text is non-empty. Dereferences to `&str`
/// for convenient access to the underlying text content.
///
/// # Reference
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
pub struct WordText(pub NonEmptyString);

impl WordText {
    /// Builds `WordText` when `text` is non-empty.
    pub fn new(text: impl AsRef<str>) -> Option<Self> {
        NonEmptyString::new(text).map(Self)
    }

    /// Builds `WordText` without runtime emptiness checks.
    pub fn new_unchecked(text: impl AsRef<str>) -> Self {
        Self(NonEmptyString::new_unchecked(text))
    }
}

impl std::ops::Deref for WordText {
    type Target = str;

    /// Borrows the inner non-empty text as `&str`.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for WordText {
    /// Borrows the inner non-empty text as `&str`.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Validate for WordText {
    /// Enforces non-empty lexical text for plain `Text` word segments.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        let ctx = context
            .clone()
            .with_field_label("word text")
            .with_field_error_code(ErrorCode::EmptyWordContentText);
        self.0.validate(&ctx, errors);
    }
}

/// An omitted sound within a word, written in parentheses in CHAT format.
///
/// Represents the elided portion of a word, e.g., `(be)` in `(be)cause`.
/// Wraps a [`NonEmptyString`] and dereferences to `&str`.
///
/// # Reference
///
/// - [Shortenings](https://talkbank.org/0info/manuals/CHAT.html#Shortenings)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
pub struct WordShortening(pub NonEmptyString);

impl WordShortening {
    /// Builds shortening text when `text` is non-empty.
    pub fn new(text: impl AsRef<str>) -> Option<Self> {
        NonEmptyString::new(text).map(Self)
    }

    /// Builds shortening text without runtime emptiness checks.
    pub fn new_unchecked(text: impl AsRef<str>) -> Self {
        Self(NonEmptyString::new_unchecked(text))
    }
}

impl std::ops::Deref for WordShortening {
    type Target = str;

    /// Borrows the shortening text as `&str`.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for WordShortening {
    /// Borrows the shortening text as `&str`.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Validate for WordShortening {
    /// Enforces non-empty text for shortening segments.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        let ctx = context
            .clone()
            .with_field_label("shortening text")
            .with_field_error_code(ErrorCode::EmptyWordContentText);
        self.0.validate(&ctx, errors);
    }
}

/// The type of word stress marker.
///
/// - `Primary` corresponds to the IPA primary stress symbol (`\u{02C8}`).
/// - `Secondary` corresponds to the IPA secondary stress symbol (`\u{02CC}`).
///
/// # Reference
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
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
pub enum WordStressMarkerType {
    /// Primary stress (`\u{02C8}`).
    Primary,
    /// Secondary stress (`\u{02CC}`).
    Secondary,
}

/// A stress marker within a word (primary or secondary).
///
/// Stress markers indicate lexical stress on syllables. They are distinct from
/// CA prosodic elements and are not included in cleaned text.
///
/// # Reference
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
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
pub struct WordStressMarker {
    /// The kind of stress (primary or secondary).
    pub marker_type: WordStressMarkerType,
    /// Source span for error reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<crate::Span>,
}

impl WordStressMarker {
    /// Builds a stress marker with no span metadata.
    pub fn new(marker_type: WordStressMarkerType) -> Self {
        Self {
            marker_type,
            span: None,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for WordStressMarker {
    /// Writes the primary (`ˈ`) or secondary (`ˌ`) stress marker.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self.marker_type {
            WordStressMarkerType::Primary => w.write_char('\u{02C8}'),
            WordStressMarkerType::Secondary => w.write_char('\u{02CC}'),
        }
    }
}

impl Validate for WordStressMarker {
    /// Marker-level semantic checks are performed at word-structure validation time.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

/// Syllable lengthening marker (`:`) within a word.
///
/// Indicates that the preceding syllable is lengthened in speech (e.g., `bana:nas`).
/// Not included in cleaned text.
///
/// # Reference
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
#[derive(
    Clone,
    Copy,
    Default,
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
pub struct WordLengthening {
    /// Number of colons (`:` = 1, `::` = 2, `:::` = 3).
    #[serde(
        default = "WordLengthening::default_count",
        skip_serializing_if = "WordLengthening::is_one"
    )]
    pub count: u8,
    /// Source span for error reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<crate::Span>,
}

impl WordLengthening {
    /// Builds a lengthening marker with count 1 and no span metadata.
    pub fn new() -> Self {
        Self {
            count: 1,
            span: None,
        }
    }

    /// Builds a lengthening marker with a specific colon count.
    pub fn with_count(count: u8) -> Self {
        Self {
            count: count.max(1),
            span: None,
        }
    }

    fn default_count() -> u8 {
        1
    }
    fn is_one(count: &u8) -> bool {
        *count <= 1
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for WordLengthening {
    /// Writes the syllable lengthening marker(s).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for _ in 0..self.count.max(1) {
            w.write_char(':')?;
        }
        Ok(())
    }
}

impl Validate for WordLengthening {
    /// Marker-level semantic checks are performed at word-structure validation time.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

/// Pause between syllables (`^`) within a word.
///
/// Indicates a noticeable pause between syllables (e.g., `o^ver`).
/// Not included in cleaned text.
///
/// # Reference
///
/// - [Words](https://talkbank.org/0info/manuals/CHAT.html#Words)
#[derive(
    Clone,
    Copy,
    Default,
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
pub struct WordSyllablePause {
    /// Source span for error reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<crate::Span>,
}

impl WordSyllablePause {
    /// Builds a syllable-pause marker with no span metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for WordSyllablePause {
    /// Writes the syllable pause marker (`^`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('^')
    }
}

impl Validate for WordSyllablePause {
    /// Marker-level semantic checks are performed at word-structure validation time.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

/// Compound marker (+) indicating word-internal compound boundary.
///
/// This marker separates components within a compound word (e.g., `ice+cream`, `wai4+yu3`).
/// It is metadata about word structure and is NOT included in `cleaned_text`.
///
/// # CHAT Format Reference
///
/// - [Compounds](https://talkbank.org/0info/manuals/CHAT.html#Compounds)
#[derive(
    Clone,
    Copy,
    Default,
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
pub struct WordCompoundMarker {
    /// Source span for error reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<crate::Span>,
}

impl WordCompoundMarker {
    /// Builds a compound-boundary marker with no span metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for WordCompoundMarker {
    /// Writes the compound boundary marker (`+`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('+')
    }
}

impl Validate for WordCompoundMarker {
    /// Marker-level semantic checks are performed at word-structure validation time.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

/// Clitic boundary marker (`~`).
///
/// Marks morphological clitic attachment within a word on the main tier.
/// Common in Arabic (`le~ha`, `bba~hum`), Spanish (`nin~o`), and other languages.
/// NOT included in `cleaned_text`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Clitics>
#[derive(
    Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct WordCliticBoundary {
    /// Source span for error reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[semantic_eq(skip)]
    pub span: Option<crate::Span>,
}

impl WordCliticBoundary {
    /// Builds a clitic boundary marker with no span metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = Some(span);
        self
    }
}

impl WriteChat for WordCliticBoundary {
    /// Writes the clitic boundary marker (`~`).
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('~')
    }
}

impl Validate for WordCliticBoundary {
    /// Marker-level semantic checks are performed at word-structure validation time.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

/// Marker for underline begin/end control characters.
///
/// Carries a source span for error reporting but serializes as `null` (adjacently tagged)
/// or `{}` → nothing extra (internally tagged) to preserve backward-compatible JSON.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
#[derive(Clone, Debug, Default, PartialEq, JsonSchema, SemanticEq, SpanShift)]
pub struct UnderlineMarker {
    /// Source span for error reporting (not serialized to JSON)
    #[schemars(skip)]
    pub span: crate::Span,
}

impl serde::Serialize for UnderlineMarker {
    /// Serializes underline markers as unit values for backward-compatible JSON shape.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_unit()
    }
}

impl<'de> serde::Deserialize<'de> for UnderlineMarker {
    /// Accepts legacy underline marker encodings (`null` or empty map) during decode.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Accept null or empty map
        /// Visitor that normalizes all legacy encodings into a default marker.
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UnderlineMarker;
            /// Describes accepted JSON shapes for diagnostics.
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("null or empty map")
            }
            /// Decodes unit/null-like values.
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UnderlineMarker::default())
            }
            /// Decodes explicit JSON `null`.
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UnderlineMarker::default())
            }
            /// Decodes legacy empty-object form.
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                _map: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(UnderlineMarker::default())
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl UnderlineMarker {
    /// Builds an underline marker with dummy span metadata.
    pub fn new() -> Self {
        Self {
            span: crate::Span::DUMMY,
        }
    }

    /// Sets source span metadata.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Builds an underline marker from explicit span metadata.
    pub fn from_span(span: crate::Span) -> Self {
        Self { span }
    }
}

impl Validate for UnderlineMarker {
    /// Underline marker pairing is validated at utterance-level structure checks.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}

/// Backward-compatible type alias
pub type WordUnderlineBegin = UnderlineMarker;
/// Backward-compatible type alias
pub type WordUnderlineEnd = UnderlineMarker;
