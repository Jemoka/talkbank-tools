//! Main-tier to `%sin` alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::helpers::{TierDomain, TierPosition, to_chat_display_string as to_string};
use super::indices::{MainWordIndex, SinItemIndex};
use super::traits::{AlignableTier, TierAlignmentResult, positional_align};
use super::types::AlignmentPair;
use crate::model::{MainTier, SinTier};
use crate::{ErrorCode, ParseError, Span};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Typed pair for main↔`%sin` alignment.
pub type SinAlignmentPair = AlignmentPair<MainWordIndex, SinItemIndex>;

/// Result of aligning main tier words to %sin tier gesture/sign tokens.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct SinAlignment {
    /// Alignment pairs ([`MainWordIndex`]↔[`SinItemIndex`])
    pub pairs: Vec<SinAlignmentPair>,

    /// Errors produced while checking `%sin` count/position alignment.
    pub errors: Vec<ParseError>,
}

impl SinAlignment {
    /// Create an empty alignment with no pairs or errors.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Append an alignment pair.
    pub fn with_pair(mut self, pair: SinAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Append an alignment error.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Returns `true` when no alignment diagnostics were emitted.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for SinAlignment {
    /// Builds an empty main-to-`%sin` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

impl TierAlignmentResult for SinAlignment {
    type Pair = SinAlignmentPair;

    fn pairs(&self) -> &[SinAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: SinAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
}

impl AlignableTier for SinTier {
    type Source = MainWordIndex;
    type Target = SinItemIndex;
    const DOMAIN: TierDomain = TierDomain::Sin;

    fn tier_name(&self) -> &str {
        "%sin tier"
    }

    fn target_count(&self) -> usize {
        self.len()
    }

    fn extract_target_items(&self) -> Vec<TierPosition> {
        self.items
            .iter()
            .map(|token| TierPosition {
                text: to_string(token),
                description: None,
            })
            .collect()
    }

    fn span(&self) -> Span {
        self.span
    }

    fn error_code_too_few(&self) -> ErrorCode {
        ErrorCode::SinCountMismatchTooFew
    }

    fn error_code_too_many(&self) -> ErrorCode {
        ErrorCode::SinCountMismatchTooMany
    }

    fn suggestion_too_few(&self) -> &str {
        "Add gesture/sign tokens to %sin tier to match main tier words"
    }

    fn suggestion_too_many(&self) -> &str {
        "Remove extra gesture/sign tokens from %sin tier"
    }
}

/// Align main-tier content to `%sin` tokens using 1:1 positional pairing.
///
/// Uses the generic [`positional_align`] algorithm via the [`AlignableTier`]
/// implementation on [`SinTier`].
pub fn align_main_to_sin(main: &MainTier, sin: &SinTier) -> SinAlignment {
    let (pairs, errors) = positional_align(main, sin);
    SinAlignment { pairs, errors }
}

// =============================================================================
// Tests for %sin alignment
// =============================================================================

#[cfg(test)]
mod sin_alignment_tests {
    use super::*;
    use crate::Span;
    use crate::model::{SinItem, SinToken, Terminator, UtteranceContent, Word};

    /// Accepts perfectly matched `%sin` and main-tier token counts.
    #[test]
    fn test_sin_alignment_perfect_match() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::new_unchecked("one", "one"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("two", "two"))),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let sin = SinTier::new(vec![
            SinItem::Token(SinToken::new_unchecked("g:toy:dpoint")),
            SinItem::Token(SinToken::new_unchecked("0")),
        ]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 2); // 2 words (terminator not in %sin)
        assert!(alignment.errors.is_empty());
        assert!(alignment.pairs.iter().all(|p| p.is_complete()));
    }

    /// Emits too-few `%sin` diagnostics when main tier has extra alignable items.
    #[test]
    fn test_sin_alignment_main_longer() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::new_unchecked("one", "one"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("two", "two"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("three", "three"))),
            ],
            Terminator::Period { span: Span::DUMMY },
        );

        let sin = SinTier::new(vec![
            SinItem::Token(SinToken::new_unchecked("g:toy:dpoint")),
            SinItem::Token(SinToken::new_unchecked("0")),
        ]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 3); // 2 matched + 1 placeholder
        assert!(!alignment.errors.is_empty());
        assert_eq!(alignment.errors.len(), 1);
        assert_eq!(alignment.errors[0].code.as_str(), "E718");
    }

    /// Emits too-many `%sin` diagnostics when `%sin` has extra tokens.
    #[test]
    fn test_sin_alignment_sin_longer() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
                "one", "one",
            )))],
            Terminator::Period { span: Span::DUMMY },
        );

        let sin = SinTier::new(vec![
            SinItem::Token(SinToken::new_unchecked("g:toy:dpoint")),
            SinItem::Token(SinToken::new_unchecked("0")),
        ]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 2); // 1 matched + 1 placeholder
        assert!(!alignment.errors.is_empty());
        assert_eq!(alignment.errors.len(), 1);
        assert_eq!(alignment.errors[0].code.as_str(), "E719");
    }

    /// Treats literal `0` `%sin` placeholders as valid alignable tokens.
    #[test]
    fn test_sin_alignment_all_zeros() {
        let main = MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::new_unchecked("what", "what"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("shall", "shall"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("we", "we"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("get", "get"))),
            ],
            Terminator::Question { span: Span::DUMMY },
        );

        let sin = SinTier::new(vec![
            SinItem::Token(SinToken::new_unchecked("0")),
            SinItem::Token(SinToken::new_unchecked("0")),
            SinItem::Token(SinToken::new_unchecked("0")),
            SinItem::Token(SinToken::new_unchecked("0")),
        ]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 4);
        assert!(alignment.errors.is_empty());
    }

    /// Accepts a common gesture token example (`g:toy:dpoint`) with one word.
    #[test]
    fn test_sin_alignment_gesture_example() {
        // Child says "junk" while pointing at toy
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
                "junk", "junk",
            )))],
            Terminator::Period { span: Span::DUMMY },
        );

        let sin = SinTier::new(vec![SinItem::Token(SinToken::new_unchecked(
            "g:toy:dpoint",
        ))]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 1);
        assert!(alignment.errors.is_empty());
    }

    /// Accepts empty-on-empty alignment without diagnostics.
    #[test]
    fn test_sin_alignment_empty() {
        let main = MainTier::new("CHI", vec![], Terminator::Period { span: Span::DUMMY });
        let sin = SinTier::new(vec![]);

        let alignment = align_main_to_sin(&main, &sin);

        assert_eq!(alignment.pairs.len(), 0);
        assert!(alignment.errors.is_empty());
    }
}
