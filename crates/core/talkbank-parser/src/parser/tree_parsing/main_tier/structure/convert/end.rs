//! Terminator/postcode extraction for `main_tier` conversion.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::{TIER_BODY, UTTERANCE_END};
use tree_sitter::Node;

use super::super::parse_utterance_end;
use super::{EndData, handle_error_node, report_missing_child};

/// Parse utterance-end data (terminator, postcodes, optional bullet).
pub(super) fn parse_end(
    node: Node,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
    mut idx: usize,
) -> EndData {
    let child_count = node.child_count();
    let mut terminator = None;
    let mut postcodes = Vec::new();
    let mut bullet = None;

    // In unified grammar, tier_body contains utterance_end as its last child
    // Check if previous child was tier_body, and if so, look inside it
    if idx > 0
        && let Some(prev_child) = node.child((idx - 1) as u32)
        && prev_child.kind() == TIER_BODY
    {
        // utterance_end is INSIDE tier_body, find it there
        let mut tb_cursor = prev_child.walk();
        for tb_child in prev_child.children(&mut tb_cursor) {
            if tb_child.kind() == UTTERANCE_END {
                let ((term, posts, bull), utterance_end_errors) =
                    parse_utterance_end(tb_child, source);
                terminator = term;
                postcodes = posts;
                bullet = bull;
                errors.report_vec(utterance_end_errors);
                return EndData {
                    terminator,
                    postcodes,
                    bullet,
                    idx, // Return same idx since we looked inside tier_body
                };
            }
        }
        // tier_body exists but no utterance_end found inside
        report_missing_child(
            original_input,
            errors,
            ErrorCode::MissingTerminator,
            "Missing terminator in tier_body",
        );
        return EndData {
            terminator,
            postcodes,
            bullet,
            idx,
        };
    }

    // Legacy path: utterance_end as sibling (old grammar)
    if idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            if handle_error_node(child, source, errors, &mut idx) {
                // handled error node
            } else if child.kind() == UTTERANCE_END {
                let ((term, posts, bull), utterance_end_errors) =
                    parse_utterance_end(child, source);
                terminator = term;
                postcodes = posts;
                bullet = bull;
                errors.report_vec(utterance_end_errors);
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::MissingTerminator,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!(
                        "Expected 'utterance_end' at position 5 of main_tier, found '{}'",
                        child.kind()
                    ),
                ));
                idx += 1;
            }
        } else {
            errors.report(ParseError::new(
                ErrorCode::MissingTerminator,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                "Failed to access child at position 5 of main_tier",
            ));
            idx += 1;
        }
    } else {
        report_missing_child(
            original_input,
            errors,
            ErrorCode::MissingTerminator,
            "Missing terminator in main tier",
        );
    }

    EndData {
        terminator,
        postcodes,
        bullet,
        idx,
    }
}
