//! Parser for `%add` (addressee) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::AddTier;
use tree_sitter::Node;

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%add` tier node.
///
/// **Grammar Rule:**
/// ```text
/// add_dependent_tier: seq('%', 'add', colon, tab, text_with_bullets, newline)
/// ```
pub fn parse_add_tier(node: Node, source: &str, errors: &impl ErrorSink) -> AddTier {
    let span = tier_span(node);
    let content = parse_text_tier_content(
        node,
        source,
        errors,
        "add_dependent_tier",
        "Missing content in %add tier",
    );
    AddTier::new(content).with_span(span)
}
