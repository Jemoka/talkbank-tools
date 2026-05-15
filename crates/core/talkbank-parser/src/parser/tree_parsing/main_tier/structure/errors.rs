//! Error collection helpers for main-tier structural parsing.
//!
//! These routines walk CST nodes, normalize wrapper offsets back to caller
//! coordinates, and preserve detailed `ErrorContext` for downstream reporting.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::error::{ErrorCode, ErrorContext, ParseError, ParseErrors, Severity, SourceLocation};
use crate::parser::tree_parsing::helpers::analyze_error_node;
use tree_sitter::Node;

/// Collect errors from parse tree (generic version)
///
/// # Arguments
/// * `offset` - Byte offset of original_input within wrapped_source (calculated by caller)
pub fn collect_tree_errors(
    node: Node,
    _wrapped_source: &str,
    original_input: &str,
    offset: usize,
    errors: &mut ParseErrors,
) {
    if node.is_missing() {
        let byte_pos = node.start_byte().saturating_sub(offset);
        let message = format!("Missing '{}'", node.kind());
        errors.push(
            ParseError::new(
                ErrorCode::MissingNode,
                Severity::Error,
                SourceLocation::from_offsets(byte_pos, byte_pos),
                ErrorContext::new(
                    original_input,
                    byte_pos..byte_pos.min(original_input.len()),
                    "",
                ),
                message,
            )
            .with_suggestion(format!("Add missing {}", node.kind())),
        );
    }

    if node.is_error() {
        // Use the wrapped_source (not original_input) for node.utf8_text() to work correctly
        let mut error = analyze_error_node(node, _wrapped_source, "parse tree");

        // Calculate adjusted offsets for original_input
        let start = node.start_byte().saturating_sub(offset);
        let end = node
            .end_byte()
            .saturating_sub(offset)
            .min(original_input.len());

        // Update the error location to match the actual position in original_input
        error.location = SourceLocation::from_offsets(start, end);

        // Update the ErrorContext to use original_input text instead of wrapped_source
        let found = if start < original_input.len() {
            &original_input[start..end.min(original_input.len())]
        } else {
            ""
        };
        error.context = Some(ErrorContext::new(original_input, start..end, found));

        // Debug output for specific test case
        if found.contains('風') {
            tracing::debug!(
                start = start,
                end = end,
                snippet = ?found,
                "Error node '風' detected"
            );
        }
        if found == "風" {
            tracing::debug!(
                start = start,
                offset = offset,
                end = end,
                "collect_tree_errors for '風'"
            );
        }

        errors.push(error);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tree_errors(child, _wrapped_source, original_input, offset, errors);
    }
}

/// Collect errors specific to main tier parsing
///
/// # Arguments
/// * `offset` - Byte offset of original_input within wrapped_source (calculated by caller)
pub fn collect_main_tier_errors(
    node: Node,
    wrapped_source: &str,
    original_input: &str,
    offset: usize,
    errors: &mut ParseErrors,
) {
    collect_tree_errors(node, wrapped_source, original_input, offset, errors);
}
