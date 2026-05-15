//! Shared extraction helpers for `@ID` header parsing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::WHITESPACES;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Returns child `index` or reports a tree-structure error.
pub(super) fn get_child_or_report<'a>(
    parent: Node<'a>,
    index: u32,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<Node<'a>> {
    match parent.child(index) {
        Some(child) => ParseOutcome::parsed(child),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(parent.start_byte(), parent.end_byte()),
                ErrorContext::new(source, parent.start_byte()..parent.end_byte(), context),
                format!("Missing child at index {} in {}", index, context),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Advance `idx` past any `whitespaces` CST nodes.
///
/// The grammar wraps optional `@ID` fields with `optional($.whitespaces)` to
/// absorb spaces around pipe-delimited content. This helper skips those
/// whitespace nodes so the field parsers can find the next meaningful child.
pub(super) fn skip_whitespace(parent: Node, idx: &mut usize, child_count: usize) {
    while *idx < child_count {
        if let Some(child) = parent.child(*idx as u32)
            && child.kind() == WHITESPACES
        {
            *idx += 1;
            continue;
        }
        break;
    }
}

/// Extracts UTF-8 text from a node outcome, reporting failures.
///
/// Returns Some(text) if successful, None if node is missing or text extraction fails.
/// If node exists but UTF-8 extraction fails, reports an error.
pub(super) fn extract_text_with_errors(
    node_outcome: ParseOutcome<Node>,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<String> {
    let ParseOutcome::Parsed(node) = node_outcome else {
        return ParseOutcome::rejected();
    };

    match node.utf8_text(source.as_bytes()) {
        Ok(text) => ParseOutcome::parsed(text.to_string()),
        Err(e) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), context),
                format!("Failed to extract UTF-8 text: {}", e),
            ));
            ParseOutcome::rejected()
        }
    }
}
