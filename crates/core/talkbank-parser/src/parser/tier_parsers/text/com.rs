//! Parser for `%com` (comment) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ComTier;
use tree_sitter::Node;

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%com` tier node.
///
/// **Grammar Rule:**
/// ```text
/// com_dependent_tier: seq('%', 'com', colon, tab, text_with_bullets_and_pics, newline)
/// ```
pub fn parse_com_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ComTier {
    let span = tier_span(node);
    let content = parse_text_tier_content(
        node,
        source,
        errors,
        "com_dependent_tier",
        "Missing content in %com tier",
    );
    ComTier::new(content).with_span(span)
}
