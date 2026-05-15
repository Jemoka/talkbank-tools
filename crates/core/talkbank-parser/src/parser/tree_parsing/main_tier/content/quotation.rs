//! Parsing for quoted main-tier segments.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM, CONTENTS,
    FALLING_TO_LOW, FALLING_TO_MID, LEFT_DOUBLE_QUOTE, LEVEL_PITCH, NON_COLON_SEPARATOR,
    OVERLAP_POINT, RIGHT_DOUBLE_QUOTE, RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR,
    TAG_MARKER, UNMARKED_ENDING, UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::group::{convert_to_group_content, parse_nested_content};
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Converts a CST `quotation` node into `UtteranceContent`.
///
/// **Grammar Rule:**
/// ```text
/// quotation: $ => seq(
///   seq(
///     '\u201C',  // Left double quotation mark "
///     optional($.whitespaces)
///   ),
///   $.contents,
///   seq(
///     optional($.whitespaces),
///     '\u201D'   // Right double quotation mark "
///   )
/// ),
/// ```
pub(crate) fn parse_quotation_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let mut group_items: Vec<crate::model::BracketedItem> = Vec::new();
    let child_count = node.child_count();
    let mut idx = 0;

    // Position 0: Opening quote mark (LEFT DOUBLE QUOTATION MARK)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == LEFT_DOUBLE_QUOTE {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected opening quote (U+201C) at position 0 of quotation, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Optional whitespace after opening quote - skip it (not semantic content)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        idx += 1;
    }

    // Parse contents
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == CONTENTS {
            group_items = parse_quotation_contents_items(child, source, errors);
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!("Expected 'contents' in quotation, found '{}'", child.kind()),
            ));
            idx += 1;
        }
    }

    // Optional whitespace before closing quote - skip it (not semantic content)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        idx += 1;
    }

    // Position last: Closing quote mark (RIGHT DOUBLE QUOTATION MARK)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == RIGHT_DOUBLE_QUOTE {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected closing quote (U+201D) in quotation, found '{}'",
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
                        "Unexpected extra child '{}' at position {} of quotation",
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

    // Create quotation - no space tracking needed
    let bracketed = crate::model::BracketedContent::new(group_items);
    let span = crate::error::Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let quotation = crate::model::Quotation::with_span(bracketed, span);
    // Quotations have no annotations
    ParseOutcome::parsed(UtteranceContent::Quotation(quotation))
}

/// Parse contents inside quotation
///
/// **Grammar Rule:**
/// ```text
/// contents: $ => repeat1(content_item)
/// ```text
fn parse_quotation_contents_items(
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
                        "quotation contents (expected content_item)",
                    ));
                }
            }
        }
    }

    group_items
}
