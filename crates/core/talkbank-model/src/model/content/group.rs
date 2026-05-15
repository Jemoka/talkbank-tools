//! Bracketed group forms used inside main-tier content.
//!
//! This module models the four surface forms that wrap a nested
//! [`BracketedContent`] payload with different CHAT semantics:
//! - [`Group`]: `<...>` regular grouping
//! - [`PhoGroup`]: `‹...›` phonological grouping
//! - [`SinGroup`]: `〔...〕` sign/gesture grouping
//! - [`Quotation`]: `“...”` quoted speech grouping
//!
//! # CHAT Format References
//!
//! - [Group](https://talkbank.org/0info/manuals/CHAT.html#Group)
//! - [Phonological Group](https://talkbank.org/0info/manuals/CHAT.html#Phonological_Group)
//! - [Sign Group](https://talkbank.org/0info/manuals/CHAT.html#Sign_Group)
//! - [Quotation](https://talkbank.org/0info/manuals/CHAT.html#Quotation)

use super::{BracketedContent, WriteChat};
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Standard angle-bracket group (`<...>`).
///
/// This is the general-purpose group construct used with scoped annotations
/// (for example retracing markers) and in regular utterance segmentation.
///
/// # CHAT Format Examples
///
/// ```text
/// <I want> [/] I need    Retracing with repetition
/// <the dog> [//] the cat  Retracing with correction
/// <um> [/] yes            Filled pause retracing
/// ```
///
/// # References
///
/// - [Group](https://talkbank.org/0info/manuals/CHAT.html#Group)
/// - [Group Scopes](https://talkbank.org/0info/manuals/CHAT.html#Group_Scopes)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Group {
    /// Inner payload written between `<` and `>`.
    pub content: BracketedContent,
    /// Optional whitespace preserved before `>` for roundtrip fidelity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_space: Option<smol_str::SmolStr>,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub span: crate::Span,
}

impl Group {
    /// Build a group with empty trailing-space metadata and dummy span.
    ///
    /// This is the default constructor for parser output and test fixtures;
    /// preserved trailing space can be attached separately when needed.
    pub fn new(content: BracketedContent) -> Self {
        Self {
            content,
            trailing_space: None,
            span: crate::Span::DUMMY,
        }
    }

    /// Attach source span metadata used by diagnostics.
    ///
    /// Span metadata is excluded from semantic equality.
    pub fn with_span(mut self, span: crate::Span) -> Self {
        self.span = span;
        self
    }

    /// Stores trailing whitespace that appears before the closing bracket.
    pub fn with_trailing_space(mut self, space: impl Into<smol_str::SmolStr>) -> Self {
        let space = space.into();
        if !space.is_empty() {
            self.trailing_space = Some(space);
        }
        self
    }

    /// Returns `true` when the inner payload has no items.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the number of inner content items.
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

impl WriteChat for Group {
    /// Serializes `<...>` and includes preserved trailing space when present.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('<')?;
        self.content.write_chat(w)?;
        if let Some(ref space) = self.trailing_space {
            w.write_str(space)?;
        }
        w.write_char('>')
    }
}

/// Phonological group (`‹...›`) used with `%pho`-aligned content.
///
/// Unlike [`Group`], this form is used as a dedicated phonological grouping
/// marker and does not carry scoped annotation behavior.
///
/// # CHAT Format Example
///
/// ```text
/// *CHI: ‹hello there› !
/// %pho: həˈloʊ
/// ```
///
/// # References
///
/// - [Phonological Group](https://talkbank.org/0info/manuals/CHAT.html#Phonological_Group)
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct PhoGroup {
    /// Inner payload written between `‹` and `›`.
    pub content: BracketedContent,
}

impl PhoGroup {
    /// Build a phonological group from pre-parsed bracketed content.
    ///
    /// Group boundaries are semantic for `%pho` alignment and must be preserved
    /// exactly during roundtrip serialization.
    pub fn new(content: BracketedContent) -> Self {
        Self { content }
    }

    /// Returns `true` when the inner payload has no items.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the number of inner content items.
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

impl WriteChat for PhoGroup {
    /// Serializes the payload inside CHAT phonological-group delimiters.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('‹')?; // U+2039 SINGLE LEFT-POINTING ANGLE QUOTATION MARK
        self.content.write_chat(w)?;
        w.write_char('›') // U+203A SINGLE RIGHT-POINTING ANGLE QUOTATION MARK
    }
}

/// Sign/gesture group (`〔...〕`) used with `%sin`-aligned content.
///
/// This form marks sign/gesture grouping with dedicated shell-bracket syntax
/// and does not carry scoped annotation behavior.
///
/// # CHAT Format Example
///
/// ```text
/// *CHI: 〔points at dog〕 want !
/// %sin: 0 point 0
/// ```
///
/// # References
///
/// - [Sign Group](https://talkbank.org/0info/manuals/CHAT.html#Sign_Group)
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct SinGroup {
    /// Inner payload written between `〔` and `〕`.
    pub content: BracketedContent,
}

impl SinGroup {
    /// Build a sign/gesture group from pre-parsed bracketed content.
    ///
    /// Group boundaries are semantic for `%sin` alignment and must be preserved
    /// exactly during roundtrip serialization.
    pub fn new(content: BracketedContent) -> Self {
        Self { content }
    }

    /// Returns `true` when the inner payload has no items.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the number of inner content items.
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

impl WriteChat for SinGroup {
    /// Serializes the payload inside CHAT sign-group delimiters.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char('〔')?; // U+3014 LEFT TORTOISE SHELL BRACKET
        self.content.write_chat(w)?;
        w.write_char('〕') // U+3015 RIGHT TORTOISE SHELL BRACKET
    }
}

/// Quoted speech group (`“...”`).
///
/// This form preserves quoted payload boundaries as a single bracketed unit.
///
/// # CHAT Format Examples
///
/// ```text
/// She said "hello there" loudly
/// I told him "no way"
/// ```
///
/// # References
///
/// - [Quotation](https://talkbank.org/0info/manuals/CHAT.html#Quotation)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Quotation {
    /// Inner payload written between curly quote delimiters.
    pub content: BracketedContent,

    /// Source location metadata for diagnostics.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub span: Span,
}

impl Quotation {
    /// Build a quotation with dummy span metadata.
    ///
    /// This constructor is convenient when source offsets are unavailable.
    pub fn new(content: BracketedContent) -> Self {
        Self {
            content,
            span: Span::DUMMY,
        }
    }

    /// Build a quotation with explicit source span metadata.
    ///
    /// Use this when parser paths already own precise source offsets.
    pub fn with_span(content: BracketedContent, span: Span) -> Self {
        Self { content, span }
    }

    /// Returns `true` when the inner payload has no items.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the number of inner content items.
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

impl WriteChat for Quotation {
    /// Serializes the payload inside curly quote delimiters.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_char(crate::chars::LEFT_DOUBLE_QUOTE)?;
        self.content.write_chat(w)?;
        w.write_char(crate::chars::RIGHT_DOUBLE_QUOTE)
    }
}
