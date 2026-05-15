//! Parsing for nested content inside groups/quotations.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Quotation>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    BASE_CONTENT_ITEM, CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA,
    CONTENT_ITEM, FALLING_TO_LOW, FALLING_TO_MID, GROUP_WITH_ANNOTATIONS, LEVEL_PITCH,
    MAIN_PHO_GROUP, MAIN_SIN_GROUP, NON_COLON_SEPARATOR, OVERLAP_POINT, QUOTATION, RISING_TO_HIGH,
    RISING_TO_MID, SEMICOLON, SEPARATOR, TAG_MARKER, UNMARKED_ENDING, UPTAKE_SYMBOL,
    VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::base::{parse_base_content, parse_overlap_point};
use super::super::pho_group::parse_pho_group_content;
use super::super::quotation::parse_quotation_content;
use super::super::sin_group::parse_sin_group_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::parse_separator_like;

/// Parse nested content items (groups, quotations) while preserving their CHAT semantics.
///
/// Called on a `content_item` node, this routine steps through each child, validates the union of the
/// CDT (content/separator/overlap) types, and routes them to the parser functions that understand how
/// the CHAT manuals describe groups, quotations, and overlapping markers. Its behavior mirrors the
/// `contents` grammar rule as documented in the Grammar section so callers can rely on the same tree
/// shapes when migrating between the Rust parser and legacy implementations.
///
/// **Grammar Rule:**
/// ```text
/// contents: $ => repeat1(choice(whitespaces, content_item, separator, overlap_point))
/// content_item: $ => choice(base_content_item, group_with_annotations, quotation, ...)
/// ```
pub(crate) fn parse_nested_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<UtteranceContent> {
    let child_count = node.child_count();
    let mut results = Vec::new();

    for idx in 0..child_count {
        let Some(child) = node.child(idx as u32) else {
            continue;
        };

        match child.kind() {
            // content_item is a wrapper — recurse through parse_content_item_nested
            // which iterates its children and dispatches to the right handler.
            CONTENT_ITEM => {
                if let ParseOutcome::Parsed(content) =
                    parse_content_item_nested(child, source, errors)
                {
                    results.push(content);
                }
            }
            // Expected: whitespace between content items (no model representation needed)
            WHITESPACES => {}
            SEPARATOR
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
                if let ParseOutcome::Parsed(sep) = parse_separator_like(child, source, errors) {
                    results.push(UtteranceContent::Separator(sep));
                }
            }
            // Bare content types: tree-sitter exposes these directly as children of
            // content_item without an extra wrapper level. Call each handler directly.
            BASE_CONTENT_ITEM => {
                if let ParseOutcome::Parsed(content) = parse_base_content(child, source, errors) {
                    results.push(content);
                }
            }
            GROUP_WITH_ANNOTATIONS => {
                if let ParseOutcome::Parsed(content) =
                    super::parser::parse_group_content(child, source, errors)
                {
                    results.push(content);
                }
            }
            QUOTATION => {
                if let ParseOutcome::Parsed(content) =
                    parse_quotation_content(child, source, errors)
                {
                    results.push(content);
                }
            }
            MAIN_PHO_GROUP => {
                if let ParseOutcome::Parsed(content) =
                    parse_pho_group_content(child, source, errors)
                {
                    results.push(content);
                }
            }
            MAIN_SIN_GROUP => {
                if let ParseOutcome::Parsed(content) =
                    parse_sin_group_content(child, source, errors)
                {
                    results.push(content);
                }
            }
            OVERLAP_POINT => {
                if let ParseOutcome::Parsed(content) = parse_overlap_point(child, source, errors) {
                    results.push(content);
                }
            }
            _ => errors.report(unexpected_node_error(child, source, "nested content")),
        }
    }

    results
}

/// Parse one nested `content_item` wrapper into `UtteranceContent`.
///
/// The helper ensures whitespace, separators, and each content category (base group, quotation, pho/sin groups)
/// are processed exactly once. It keeps the structure defined in the manual’s Group and Quotation sections intact
/// by rejecting unexpected nodes.
fn parse_content_item_nested(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();

    for idx in 0..child_count {
        let Some(child) = node.child(idx as u32) else {
            continue;
        };

        match child.kind() {
            BASE_CONTENT_ITEM => return parse_base_content(child, source, errors),
            GROUP_WITH_ANNOTATIONS => {
                return super::parser::parse_group_content(child, source, errors);
            }
            QUOTATION => return parse_quotation_content(child, source, errors),
            MAIN_PHO_GROUP => return parse_pho_group_content(child, source, errors),
            MAIN_SIN_GROUP => {
                return parse_sin_group_content(child, source, errors);
            }
            OVERLAP_POINT => {
                use super::super::parse_overlap_point;
                return parse_overlap_point(child, source, errors);
            }
            SEPARATOR => {
                return parse_separator_like(child, source, errors)
                    .map(UtteranceContent::Separator);
            }
            NON_COLON_SEPARATOR
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
                return parse_separator_like(child, source, errors)
                    .map(UtteranceContent::Separator);
            }
            // Expected: whitespace around content items (no model representation needed)
            WHITESPACES => continue,
            _ => {
                errors.report(unexpected_node_error(child, source, "content_item"));
                return ParseOutcome::rejected();
            }
        }
    }

    ParseOutcome::rejected()
}

/// Parse base content inside group nodes.
///
/// Groups are described in the CHAT manual as `group_content` nodes wrapping either base content, a quotation,
/// or nested pho/sin groups. This helper enforces that expectation while routing to the correct parser for the
/// nested content.
///
/// **Expected:** group_content node contains a single child (base_content, main_pho_group, or quotation)
#[allow(dead_code)]
pub(crate) fn parse_base_content_in_group(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();

    // Position 0: base_content or main_pho_group (should be exactly one child)
    if child_count == 0 {
        return ParseOutcome::rejected();
    }

    if let Some(child) = node.child(0u32) {
        let content = match child.kind() {
            BASE_CONTENT_ITEM => parse_base_content(child, source, errors),
            MAIN_PHO_GROUP => parse_pho_group_content(child, source, errors),
            QUOTATION => parse_quotation_content(child, source, errors),
            _ => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!(
                        "Expected 'base_content_item', 'main_pho_group', or 'quotation' at position 0 of group_content, found '{}'",
                        child.kind()
                    ),
                ));
                ParseOutcome::rejected()
            }
        };

        // Check for unexpected extra children
        if child_count > 1 {
            for idx in 1..child_count {
                if let Some(extra) = node.child(idx as u32) {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(extra.start_byte(), extra.end_byte()),
                        ErrorContext::new(source, extra.start_byte()..extra.end_byte(), ""),
                        format!(
                            "Unexpected extra child '{}' at position {} of group_content",
                            extra.kind(),
                            idx
                        ),
                    ));
                }
            }
        }

        return content;
    }

    ParseOutcome::rejected()
}
