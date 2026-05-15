//! Parser for `%gpx` tiers.
//!
//! `%gpx` content is represented as text-with-bullets in the same family as
//! `%com`/`%exp`/`%add`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gems>

use talkbank_model::ErrorSink;
use talkbank_model::model::GpxTier;
use tree_sitter::Node;

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%gpx` tier node.
///
/// **Grammar Rule:**
/// ```text
/// gpx_dependent_tier: seq('%', 'gpx', colon, tab, text_with_bullets, newline)
/// ```
pub fn parse_gpx_tier(node: Node, source: &str, errors: &impl ErrorSink) -> GpxTier {
    let span = tier_span(node);
    let content = parse_text_tier_content(
        node,
        source,
        errors,
        "gpx_dependent_tier",
        "Missing content in %gpx tier",
    );
    GpxTier::new(content).with_span(span)
}
