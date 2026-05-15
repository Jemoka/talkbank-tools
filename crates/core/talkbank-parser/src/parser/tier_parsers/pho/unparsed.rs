//! Fallback parsing for unparsed `%mod`/phonology-style dependent tiers.
//!
//! Tree-sitter may classify some phonology-like tiers under
//! `unparsed_dependent_tier`; this module projects those nodes into `PhoTier`
//! by extracting and tokenizing the raw `free_text` payload.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

use talkbank_model::model::{PhoItem, PhoTier, PhoTierType, PhoWord};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Converts `%mod` from an `unparsed_dependent_tier` node.
///
/// %mod uses unparsed_dependent_tier grammar, so content is "free_text" node.
pub fn parse_mod_tier_from_unparsed(node: Node, source: &str, errors: &impl ErrorSink) -> PhoTier {
    parse_unparsed_pho_tier(node, source, PhoTierType::Mod, errors)
}

/// Converts phonological-style content from `unparsed_dependent_tier`.
///
/// **Grammar Rule:**
/// ```text
/// unparsed_dependent_tier: seq('%', /[a-z]+/, colon, tab, free_text, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. tier name (position 1) - e.g., "mod", "upho"
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. free_text (position 4) - content
/// 6. newline (position 5)
///
/// Since content is "free_text" (unstructured), we split on whitespace manually.
fn parse_unparsed_pho_tier(
    node: Node,
    source: &str,
    tier_type: PhoTierType,
    errors: &impl ErrorSink,
) -> PhoTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    // For unparsed_dependent_tier: seq('%', /[a-z]+/, colon, tab, free_text, newline)
    // Note: /[a-z]+/ (tier name regex) does NOT create a separate node!
    // Actual positions:
    // Position 0: '%'
    // Position 1: colon
    // Position 2: tab
    // Position 3: free_text (content) <-- THE CONTENT IS HERE!
    // Position 4: newline

    // Position 3 should be the "free_text" node with the content
    let content_node = match node.child(3u32) {
        Some(child) if !child.is_missing() => child,
        _ => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    source,
                    node.start_byte()..node.end_byte(),
                    "unparsed_dependent_tier",
                ),
                "Missing content node in unparsed %pho/%mod tier",
            ));
            return PhoTier::new(tier_type, Vec::new()).with_span(span);
        }
    };
    let content_text = match content_node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(content_node.start_byte(), content_node.end_byte()),
                ErrorContext::new(
                    source,
                    content_node.start_byte()..content_node.end_byte(),
                    "unparsed_dependent_tier_content",
                ),
                format!("Invalid UTF-8 in unparsed %pho/%mod tier content: {}", err),
            ));
            return PhoTier::new(tier_type, Vec::new()).with_span(span);
        }
    };

    let items: Vec<PhoItem> = if content_text.is_empty() {
        Vec::new()
    } else {
        content_text
            .split_whitespace()
            .map(|s| PhoItem::Word(PhoWord::new(s)))
            .collect()
    };

    PhoTier::new(tier_type, items).with_span(span)
}
