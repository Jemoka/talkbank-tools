//! `%sin` dependent-tier model for gesture and sign annotations.
//!
//! CHAT reference anchors:
//! - [Gestures](https://talkbank.org/0info/manuals/CHAT.html#Gestures)
//! - [Sign language coding](https://talkbank.org/0info/manuals/CHAT.html#SignLanguage)

use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::ops::{Deref, DerefMut};
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::WriteChat;
use super::{SinItem, SinToken};

/// Gesture and sign annotation tier (%sin).
///
/// Records gestures, pointing, and sign language that accompany speech.
/// Each token corresponds to one word in the main tier, capturing the
/// non-verbal communication that occurs simultaneously with speech.
///
/// # Alignment
///
/// The %sin tier aligns 1-to-1 with alignable main tier content:
/// - One token per word (excluding retraces, pauses, events)
/// - Use `0` for words without gestures
/// - Terminator gets its own token (typically `.` or `0`)
///
/// # Token Format
///
/// Gesture codes follow the pattern `g:referent:type`:
/// - **g**: Gesture marker
/// - **referent**: Object/person the gesture refers to (e.g., "ball", "mom", "toy")
/// - **type**: Gesture type (e.g., "dpoint" for deictic point, "hold", "give")
///
/// Multiple gestures for one word use special brackets: `〔g:x g:y〕`
///
/// # Research Applications
///
/// - **Child development**: Track gesture-speech integration
/// - **Communication disorders**: Document gestural compensation
/// - **Sign language**: Record sign-speech bilingualism
/// - **Multimodal analysis**: Study gesture-speech timing and meaning
///
/// # CHAT Manual Reference
///
/// - [Gesture Coding](https://talkbank.org/0info/manuals/CHAT.html#Gestures)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{SinItem, SinTier, SinToken};
///
/// // Child points to ball while saying "ball"
/// let sin = SinTier::new(vec![
///     SinItem::Token(SinToken::new_unchecked("g:ball:dpoint")),
///     SinItem::Token(SinToken::new_unchecked("0"))  // No gesture on terminator
/// ]);
/// ```
///
/// **CHAT format:**
/// ```text
/// *CHI: ball .
/// %sin: g:ball:dpoint 0
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct SinTier {
    /// Gesture/sign items aligned with main tier.
    ///
    /// Each item represents the gesture(s) accompanying one word:
    /// - `SinItem::Token` - Single gesture or no gesture ("0")
    /// - `SinItem::SinGroup` - Multiple gestures
    ///
    /// Item order matches main tier word order.
    pub items: SinItems,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl SinTier {
    /// Constructs a `%sin` tier from parsed gesture/sign items.
    pub fn new(items: Vec<SinItem>) -> Self {
        Self {
            items: items.into(),
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Legacy convenience constructor from plain `%sin` token strings.
    ///
    /// Each token becomes `SinItem::Token`. Prefer [`Self::new`] when callers
    /// already parsed grouped or structured `%sin` forms.
    pub fn from_tokens(tokens: Vec<String>) -> Self {
        let items: Vec<SinItem> = tokens
            .into_iter()
            .map(|text| SinItem::Token(SinToken::new_unchecked(text)))
            .collect();
        Self {
            items: items.into(),
            span: Span::DUMMY,
        }
    }

    /// Number of alignment slots represented in this tier.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when there are no `%sin` items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl WriteChat for SinTier {
    /// Serializes one full `%sin` line.
    fn write_chat<W: FmtWrite>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("%sin:\t")?;
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }
        Ok(())
    }
}

/// Ordered `%sin` items aligned with main-tier tokens.
///
/// # Reference
///
/// - [Gestures](https://talkbank.org/0info/manuals/CHAT.html#Gestures)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct SinItems(pub Vec<SinItem>);

impl SinItems {
    /// Wraps ordered `%sin` items without reinterpreting alignment.
    pub fn new(items: Vec<SinItem>) -> Self {
        Self(items)
    }

    /// Returns `true` when this item list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for SinItems {
    type Target = Vec<SinItem>;

    /// Borrows the underlying `%sin` item vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SinItems {
    /// Mutably borrows the underlying `%sin` item vector.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<SinItem>> for SinItems {
    /// Wraps `%sin` items without additional allocation.
    fn from(items: Vec<SinItem>) -> Self {
        Self(items)
    }
}

impl crate::validation::Validate for SinItems {
    /// Cross-tier `%sin` checks run where main-tier alignment context is available.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        _errors: &impl crate::ErrorSink,
    ) {
    }
}
