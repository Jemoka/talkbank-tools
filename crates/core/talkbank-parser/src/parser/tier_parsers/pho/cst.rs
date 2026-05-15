//! CST-driven parsing for `%pho` and `%mod` tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

use crate::node_types as kind;
use talkbank_model::model::dependent_tier::PhoGroupWords;
use talkbank_model::model::{PhoItem, PhoTier, PhoTierType, PhoWord};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

use super::groups::extract_pho_group_items;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::check_not_missing;

/// Parse a `%pho` tier from a tree-sitter node.
///
/// **Grammar Rule:**
/// ```text
/// pho_dependent_tier: seq('%', 'pho', colon, tab, pho_groups, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'pho' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. pho_groups (position 4)
/// 6. newline (position 5)
pub fn parse_pho_tier(node: Node, source: &str, errors: &impl ErrorSink) -> PhoTier {
    parse_pho_tier_inner(node, source, PhoTierType::Pho, errors)
}

/// Parse a `%mod` tier from a tree-sitter node.
///
/// **Grammar Rule:**
/// ```text
/// mod_dependent_tier: seq('%', 'mod', colon, tab, pho_groups, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'mod' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. pho_groups (position 4)
/// 6. newline (position 5)
pub fn parse_mod_tier(node: Node, source: &str, errors: &impl ErrorSink) -> PhoTier {
    parse_pho_tier_inner(node, source, PhoTierType::Mod, errors)
}

// Note: %xpho is a user-defined tier type and should be stored as unparsed.
// It is NOT treated as a real phonological tier type in the data model.
// The treesitter.rs parser correctly handles %xpho as an unparsed tier.

/// Shared implementation for `%pho` and `%mod` tier parsing.
fn parse_pho_tier_inner(
    node: Node,
    source: &str,
    tier_type: PhoTierType,
    errors: &impl ErrorSink,
) -> PhoTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let mut pho_groups = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind::PHO_GROUPS {
            pho_groups = Some(child);
            break;
        }
    }

    let pho_groups = match pho_groups {
        Some(groups) => groups,
        None => {
            return PhoTier::new(tier_type, Vec::new()).with_span(span);
        }
    };

    // Navigate pho_groups children using CST
    // Grammar: pho_groups = pho_group + repeat(whitespaces + pho_group)
    let child_count = pho_groups.child_count();
    // Pre-allocate: typically half the children are pho_group (others are whitespace)
    let mut items: Vec<PhoItem> = Vec::with_capacity(child_count / 2 + 1);
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = pho_groups.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes before processing
            if !check_not_missing(child, source, errors, "pho_groups") {
                idx += 1;
                continue;
            }

            match child.kind() {
                kind::PHO_GROUP => {
                    // pho_group is choice(pho_words, seq('‹', pho_grouped_content, '›'))
                    // Extract items from this group
                    let group_items = extract_pho_group_items(child, source, errors);
                    items.extend(group_items);
                }
                kind::WHITESPACES => {
                    // Skip whitespace separators
                }
                _ => {
                    errors.report(unexpected_node_error(child, source, "pho_groups"));
                }
            }
        }
        idx += 1;
    }

    PhoTier::new(tier_type, items).with_span(span)
}

/// Build a fallback `PhoWord` item from raw group text when detailed parsing fails.
pub(crate) fn fallback_group_as_text(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<PhoItem> {
    match node.utf8_text(source.as_bytes()) {
        Ok(text) if !text.is_empty() => {
            vec![PhoItem::Word(PhoWord::new(text))]
        }
        Ok(_) => vec![],
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "pho_group"),
                format!("Invalid UTF-8 in %pho group fallback text: {}", err),
            ));
            vec![]
        }
    }
}

/// Builds group from words for downstream use.
pub(crate) fn build_group_from_words(words: Vec<&str>) -> Vec<PhoItem> {
    if !words.is_empty() {
        let pho_words: Vec<PhoWord> = words.into_iter().map(PhoWord::new).collect();
        vec![PhoItem::Group(PhoGroupWords::new(pho_words))]
    } else {
        vec![]
    }
}
