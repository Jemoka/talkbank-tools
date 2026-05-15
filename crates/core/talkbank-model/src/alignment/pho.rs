//! Main-tier to `%pho` alignment.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::helpers::{TierDomain, TierPosition, to_chat_display_string as to_string};
use super::indices::{MainWordIndex, PhoItemIndex};
use super::traits::{AlignableTier, TierAlignmentResult, positional_align};
use super::types::AlignmentPair;
use crate::model::{MainTier, PhoTier};
use crate::{ErrorCode, ParseError, Span};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Typed pair for main↔`%pho` (and main↔`%mod`) alignment.
pub type PhoAlignmentPair = AlignmentPair<MainWordIndex, PhoItemIndex>;

/// Result of aligning main-tier units to `%pho` tokens.
///
/// `pairs` always preserves positional intent, including placeholder entries for
/// mismatches. `errors` carries user-facing diagnostics explaining why those
/// placeholders were needed.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift)]
pub struct PhoAlignment {
    /// Positional mapping rows ([`MainWordIndex`]↔[`PhoItemIndex`]).
    pub pairs: Vec<PhoAlignmentPair>,

    /// Diagnostics produced while enforcing count/position invariants.
    pub errors: Vec<ParseError>,
}

impl PhoAlignment {
    /// Creates an empty alignment accumulator.
    ///
    /// Used by the builder-style alignment loop before rows and diagnostics are appended.
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Appends one positional alignment row.
    ///
    /// This consumes and returns `Self` so call sites can chain in tight loops.
    pub fn with_pair(mut self, pair: PhoAlignmentPair) -> Self {
        self.pairs.push(pair);
        self
    }

    /// Appends one diagnostic describing an alignment mismatch.
    ///
    /// Multiple mismatches can be accumulated when callers choose to continue.
    pub fn with_error(mut self, error: ParseError) -> Self {
        self.errors.push(error);
        self
    }

    /// Returns `true` when alignment completed without mismatch diagnostics.
    ///
    /// A `true` value implies all rows in `pairs` are complete one-to-one matches.
    pub fn is_error_free(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for PhoAlignment {
    /// Builds an empty main-to-`%pho` alignment result.
    fn default() -> Self {
        Self::new()
    }
}

impl TierAlignmentResult for PhoAlignment {
    type Pair = PhoAlignmentPair;

    fn pairs(&self) -> &[PhoAlignmentPair] {
        &self.pairs
    }

    fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    fn push_pair(&mut self, pair: PhoAlignmentPair) {
        self.pairs.push(pair);
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors.push(error);
    }
}

impl AlignableTier for PhoTier {
    type Source = MainWordIndex;
    type Target = PhoItemIndex;
    const DOMAIN: TierDomain = TierDomain::Pho;

    fn tier_name(&self) -> &str {
        "%pho tier"
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
        ErrorCode::PhoCountMismatchTooFew
    }

    fn error_code_too_many(&self) -> ErrorCode {
        ErrorCode::PhoCountMismatchTooMany
    }

    fn suggestion_too_few(&self) -> &str {
        "Add phonological tokens to %pho tier to match main tier words"
    }

    fn suggestion_too_many(&self) -> &str {
        "Remove extra phonological tokens from %pho tier"
    }
}

/// Align main-tier content to `%pho` tokens using 1:1 positional pairing.
///
/// This pass enforces the `%pho` contract that each alignable main-tier unit
/// has exactly one corresponding phonological token.
///
/// Uses the generic [`positional_align`] algorithm via the [`AlignableTier`]
/// implementation on [`PhoTier`].
pub fn align_main_to_pho(main: &MainTier, pho: &PhoTier) -> PhoAlignment {
    let (pairs, errors) = positional_align(main, pho);
    PhoAlignment { pairs, errors }
}
