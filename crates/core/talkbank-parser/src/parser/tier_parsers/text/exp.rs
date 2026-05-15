//! Parser for `%exp` (explanation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ExpTier;
use tree_sitter::Node;

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%exp` tier node.
///
/// **Grammar Rule:**
/// ```text
/// exp_dependent_tier: seq('%', 'exp', colon, tab, text_with_bullets, newline)
/// ```
pub fn parse_exp_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ExpTier {
    let span = tier_span(node);
    let content = parse_text_tier_content(
        node,
        source,
        errors,
        "exp_dependent_tier",
        "Missing content in %exp tier",
    );
    ExpTier::new(content).with_span(span)
}
