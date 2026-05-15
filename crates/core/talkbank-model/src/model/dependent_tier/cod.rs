//! Coding tier (%cod) for CHAT transcripts.
//!
//! The %cod tier provides flexible coding categories for research-specific annotation.
//! Codes can apply to entire utterances or to specific words using word indices.
//!
//! # Word Index Format
//!
//! Word indices indicate which words the code applies to:
//! - **`<1>`**: Word 1
//! - **`<1+2>`**: Words 1 and 2 combined
//! - **`<1,3>`**: Words 1 and 3 (non-contiguous)
//! - **`<1-3>`**: Words 1 through 3 (range)
//! - **`<1-3,5>`**: Words 1-3 and 5
//!
//! # Format
//!
//! ```text
//! %cod:\t[<index>] code_value [<index>] code_value ...
//! ```
//!
//! Or for utterance-level coding:
//! ```text
//! %cod:\tcode_value
//! ```
//!
//! # CHAT Manual Reference
//!
//! - [Coding Tier](https://talkbank.org/0info/manuals/CHAT.html#Coding)
//!
//! # Examples
//!
//! ```text
//! *CHI: the big dog barked .
//! %cod: <1+2> DET+ADJ <3> ANIMAL <4> VERB
//!
//! *MOT: that's right !
//! %cod: POSITIVE_FEEDBACK
//! ```

use super::{BulletContent, WriteChat};
use crate::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Coding tier (%cod).
///
/// General-purpose coding tier for research-specific annotation schemes.
/// Codes can target specific words using indices or apply to the whole utterance.
/// May contain inline media bullets.
///
/// # Word Indexing
///
/// Codes can be associated with specific words using angle-bracket indices:
/// - `<1>` - Single word (word 1)
/// - `<1+2>` - Adjacent words combined
/// - `<1,3>` - Multiple non-contiguous words
/// - `<1-3>` - Word range
///
/// Word indices refer to positions in the main tier (excluding retraces/events).
///
/// # Common Uses
///
/// - Semantic role labeling
/// - Thematic coding
/// - Discourse function coding
/// - Error type classification
/// - Custom research categories
/// - Lexical category annotations
///
/// # CHAT Manual Reference
///
/// - [Coding Categories](https://talkbank.org/0info/manuals/CHAT.html#Coding)
///
/// # Examples
///
/// ```
/// use talkbank_model::model::{CodTier, BulletContent};
///
/// // Word-specific coding
/// let cod = CodTier::new(BulletContent::from_text("<1> DETERMINER <2> NOUN"));
///
/// // Utterance-level coding
/// let cod2 = CodTier::from_text("QUESTION_TAG");
/// ```
///
/// **CHAT format:**
/// ```text
/// *CHI: I want cookie .
/// %cod: <1> PRONOUN <2> MODAL <3> FOOD_ITEM
///
/// *MOT: good job !
/// %cod: PRAISE
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct CodTier {
    /// Coding content with optional word indices and inline bullets.
    pub content: BulletContent,

    /// Source span for error reporting (not serialized to JSON)
    #[serde(skip)]
    #[schemars(skip)]
    pub span: Span,
}

impl CodTier {
    /// Constructs a `%cod` tier from parsed bullet-aware content.
    pub fn new(content: BulletContent) -> Self {
        Self {
            content,
            span: Span::DUMMY,
        }
    }

    /// Sets source span metadata used in diagnostics.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Convenience constructor for simple `%cod` text payloads.
    ///
    /// Use [`Self::new`] when callers already have parsed [`BulletContent`].
    pub fn from_text(text: impl Into<smol_str::SmolStr>) -> Self {
        Self {
            content: BulletContent::from_text(text),
            span: Span::DUMMY,
        }
    }

    /// Returns `true` when no serializable coding payload is present.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Allocating helper that writes `%cod:\t...` into a `String`.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}

impl WriteChat for CodTier {
    /// Serializes one full `%cod` line.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str("%cod:\t")?;
        self.content.write_chat(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures plain coding text round-trips to `%cod` format.
    #[test]
    fn test_cod_tier_simple() {
        let tier = CodTier::from_text("general coding");
        assert_eq!(tier.to_chat(), "%cod:\tgeneral coding");
        assert!(!tier.is_empty());
    }

    /// Ensures indexed coding syntax is preserved verbatim.
    #[test]
    fn test_cod_tier_with_indices() {
        let tier = CodTier::from_text("<1> atul");
        assert_eq!(tier.to_chat(), "%cod:\t<1> atul");
        assert!(!tier.is_empty());
    }

    /// Empty text produces an empty payload according to `is_empty`.
    #[test]
    fn test_cod_tier_empty() {
        let tier = CodTier::from_text("");
        assert!(tier.is_empty());
    }
}
