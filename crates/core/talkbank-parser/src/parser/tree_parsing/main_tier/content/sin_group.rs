//! Parsing for main-tier sign/gesture groups (`〔 ... 〕`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM, CONTENTS,
    FALLING_TO_LOW, FALLING_TO_MID, LEVEL_PITCH, NON_COLON_SEPARATOR, OVERLAP_POINT,
    RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR, SIN_BEGIN_GROUP, SIN_END_GROUP,
    TAG_MARKER, UNMARKED_ENDING, UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::group::{convert_to_group_content, parse_nested_content};
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Parse a `main_sin_group` node into `UtteranceContent::SinGroup`.
pub(crate) fn parse_sin_group_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let mut group_items: Vec<crate::model::BracketedItem> = Vec::new();
    let child_count = node.child_count();
    let mut idx = 0;

    // Position 0: '〔'
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == SIN_BEGIN_GROUP {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected '〔' at position 0 of main_sin_group, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Position 1: contents
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == CONTENTS {
            group_items = parse_sin_group_contents_items(child, source, errors);
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected 'contents' at position 1 of main_sin_group, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Position 2: '〕'
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == SIN_END_GROUP {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected '〕' at position 2 of main_sin_group, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Check for unexpected extra children
    if idx < child_count {
        for extra_idx in idx..child_count {
            if let Some(extra) = node.child(extra_idx as u32) {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(extra.start_byte(), extra.end_byte()),
                    ErrorContext::new(source, extra.start_byte()..extra.end_byte(), ""),
                    format!(
                        "Unexpected extra child '{}' at position {} of main_sin_group",
                        extra.kind(),
                        extra_idx
                    ),
                ));
            }
        }
    }

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }

    let bracketed = crate::model::BracketedContent::new(group_items);
    let sin_group = crate::model::SinGroup::new(bracketed);
    // Sin groups have no annotations
    ParseOutcome::parsed(UtteranceContent::SinGroup(sin_group))
}

/// Parse the `contents` payload inside a sin group.
///
/// **Grammar Rule:**
/// ```text
/// contents: $ => repeat1(content_item)
/// ```
fn parse_sin_group_contents_items(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<crate::model::BracketedItem> {
    let child_count = node.child_count();
    // Pre-allocate: each child is typically one content item
    let mut group_items = Vec::with_capacity(child_count);
    for idx in 0..child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                CONTENT_ITEM
                | OVERLAP_POINT
                | SEPARATOR
                | NON_COLON_SEPARATOR
                | COLON
                | COMMA
                | SEMICOLON
                | TAG_MARKER
                | VOCATIVE_MARKER
                | CA_CONTINUATION_MARKER
                | UNMARKED_ENDING
                | UPTAKE_SYMBOL
                | CA_NO_BREAK
                | CA_TECHNICAL_BREAK
                | RISING_TO_HIGH
                | RISING_TO_MID
                | LEVEL_PITCH
                | FALLING_TO_MID
                | FALLING_TO_LOW => {
                    for content in parse_nested_content(child, source, errors) {
                        if let Some(group_content) = convert_to_group_content(content) {
                            group_items.push(group_content);
                        }
                    }
                }
                // Expected: whitespace between content items (no model representation needed)
                WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(
                        child,
                        source,
                        "sin_group contents (expected content_item)",
                    ));
                }
            }
        }
    }

    group_items
}
