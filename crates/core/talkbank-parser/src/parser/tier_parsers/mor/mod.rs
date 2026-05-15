//! `%mor` tier parser using tree-sitter CST.
//!
//! This module parses morphology content into `MorTier`/`MorWord` structures
//! and is used both for `%mor` tier parsing and for alignment-critical
//! utterance health checks.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

pub mod item;
pub mod tier;
pub mod word;

use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{MorTier, MorTierType};
use tree_sitter::Node;

pub use tier::parse_mor_tier_inner;

/// Converts one `%mor` tier from a CST node.
///
/// Returns [`ParseOutcome::Rejected`] when the tier has any
/// unrecoverable parse failure: a malformed item, an unrecognized
/// terminator, or no terminator at all. Diagnostics are streamed via
/// the supplied [`ErrorSink`]; the caller decides per-utterance
/// whether to mark the morphology as `BlockedByMorParseFailure` (rule
/// 6e) or skip it.
pub fn parse_mor_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<MorTier> {
    parse_mor_tier_inner(node, source, MorTierType::Mor, errors)
}
