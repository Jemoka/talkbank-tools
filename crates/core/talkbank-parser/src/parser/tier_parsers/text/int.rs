//! Parser for `%int` (intonation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::IntTier;
use tree_sitter::Node;

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%int` tier node.
///
/// **Grammar Rule:**
/// ```text
/// int_dependent_tier: seq('%', 'int', colon, tab, text_with_bullets, newline)
/// ```
pub fn parse_int_tier(node: Node, source: &str, errors: &impl ErrorSink) -> IntTier {
    let span = tier_span(node);
    let content = parse_text_tier_content(
        node,
        source,
        errors,
        "int_dependent_tier",
        "Missing content in %int tier",
    );
    IntTier::new(content).with_span(span)
}
