//! Helper routines for replacement-annotation parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Validate expected child kind while preserving recovery progress.
pub(super) fn expect_child_kind(
    _node: Node,
    source: &str,
    child: Node,
    expected: &str,
    context: &str,
    index: usize,
    errors: &impl ErrorSink,
) -> bool {
    if child.kind() == expected {
        true
    } else {
        errors.report(ParseError::new(
            ErrorCode::ReplacementParseError,
            Severity::Error,
            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
            format!(
                "Expected '{}' at position {} of {}, found '{}'",
                expected,
                index,
                context,
                child.kind()
            ),
        ));
        false
    }
}

/// Emit replacement-context diagnostic for unexpected child kinds.
pub(super) fn report_unexpected_kind(
    _node: Node,
    source: &str,
    child: Node,
    context: &str,
    message: String,
    errors: &impl ErrorSink,
) {
    errors.report(ParseError::new(
        ErrorCode::ReplacementParseError,
        Severity::Error,
        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
        format!("{} in {}", message, context),
    ));
}
