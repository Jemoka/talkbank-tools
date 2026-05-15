//! Parser for `%spa` speech-act tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::SpaTier;
use tree_sitter::Node;

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%spa` tier node.
///
/// **Grammar Rule:**
/// ```text
/// spa_dependent_tier: seq('%', 'spa', colon, tab, text_with_bullets, newline)
/// ```
pub fn parse_spa_tier(node: Node, source: &str, errors: &impl ErrorSink) -> SpaTier {
    let span = tier_span(node);
    let content = parse_text_tier_content(
        node,
        source,
        errors,
        "spa_dependent_tier",
        "Missing content in %spa tier",
    );
    SpaTier::new(content).with_span(span)
}
