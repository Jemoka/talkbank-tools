//! Shared helpers for text-like dependent tier parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::node_types::{
    ADD_TIER_PREFIX, COM_TIER_PREFIX, EXP_TIER_PREFIX, GPX_TIER_PREFIX, INT_TIER_PREFIX, NEWLINE,
    SIT_TIER_PREFIX, SPA_TIER_PREFIX, TEXT_WITH_BULLETS, TEXT_WITH_BULLETS_AND_PICS, TIER_SEP,
    WHITESPACES,
};
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use talkbank_model::model::BulletContent;
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Compute the source span for an entire tier node.
pub(super) fn tier_span(node: Node) -> Span {
    Span::new(node.start_byte() as u32, node.end_byte() as u32)
}

/// Parse the text/bullet payload of a text-like dependent tier.
pub(super) fn parse_text_tier_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
    message: &str,
) -> BulletContent {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            TEXT_WITH_BULLETS | TEXT_WITH_BULLETS_AND_PICS => {
                return parse_bullet_content(child, source, errors);
            }
            // Structural CST nodes: tier prefix, separator, and newline are part of the
            // dependent tier grammar rule but carry no semantic content
            COM_TIER_PREFIX | ADD_TIER_PREFIX | EXP_TIER_PREFIX | GPX_TIER_PREFIX
            | INT_TIER_PREFIX | SIT_TIER_PREFIX | SPA_TIER_PREFIX | TIER_SEP | NEWLINE => {
                continue;
            }
            // Expected: whitespace between structural elements (no model representation needed)
            WHITESPACES => continue,
            _ => errors.report(unexpected_node_error(child, source, context)),
        }
    }

    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), context),
        message.to_string(),
    ));
    BulletContent::from_text("")
}
