//! Recursive traversal helpers for collecting tree-sitter recovery errors.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use crate::node_types::{GRA_DEPENDENT_TIER, MOR_DEPENDENT_TIER, PHO_DEPENDENT_TIER};
use tree_sitter::Node;

use super::error_analysis::analyze_dependent_tier_error_with_context;

/// Recursively walks a subtree and collects parse errors.
pub(crate) fn check_for_errors_recursive(node: Node, source: &str, errors: &mut Vec<ParseError>) {
    check_for_errors_recursive_with_context(node, source, errors, None);
}

/// Recursively walks a subtree and tracks tier context for better diagnostics.
pub(crate) fn check_for_errors_recursive_with_context(
    node: Node,
    source: &str,
    errors: &mut Vec<ParseError>,
    tier_type: Option<&str>,
) {
    // Check for ERROR nodes (tree-sitter couldn't parse this content)
    if node.is_error() {
        errors.push(analyze_dependent_tier_error_with_context(
            node, source, tier_type,
        ));
        return;
    }

    // Check for MISSING nodes (tree-sitter inserted placeholder for required element)
    if node.is_missing() {
        let tier_context = match tier_type {
            Some(t) => format!(" in {} tier", t),
            None => String::new(),
        };
        errors.push(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!(
                "Missing required '{}'{} at byte {} (tree-sitter error recovery)",
                node.kind(),
                tier_context,
                node.start_byte()
            ),
        ));
        return;
    }

    // Determine tier type from node kind
    let new_tier_type = match node.kind() {
        MOR_DEPENDENT_TIER => Some("mor"),
        GRA_DEPENDENT_TIER => Some("gra"),
        PHO_DEPENDENT_TIER => Some("pho"),
        _ if tier_type.is_some() => tier_type, // Inherit parent tier type
        _ => None,
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_for_errors_recursive_with_context(child, source, errors, new_tier_type);
    }
}
