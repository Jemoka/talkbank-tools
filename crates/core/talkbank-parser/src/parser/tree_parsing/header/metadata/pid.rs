//! Parsing for `@PID` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>

use crate::node_types::{FREE_TEXT, PID_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::model::{Header, PidValue, WarningText};

/// Build `Header::Unknown` for malformed `@PID` input.
fn unknown_pid_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@PID".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @PID:\t<value>".to_string()),
    }
}

/// Parse PID header from tree-sitter node
///
/// **Grammar Rule:**
/// ```javascript
/// pid_header: $ => seq(
///     '@', 'PID', $.colon, $.tab,
///     $.free_text,    // Position 4
///     $.newline
/// )
/// ```
pub fn parse_pid_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a pid_header node
    if node.kind() != PID_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected pid_header node, got: {}", node.kind()),
        ));
        return unknown_pid_header(node, source, "PID header CST node had unexpected kind");
    }

    // Grammar: seq(prefix, header_sep, free_text, newline)
    let pid = if let Some(child) = find_child_by_kind(node, FREE_TEXT) {
        match child.utf8_text(source.as_bytes()) {
            Ok(text) => text.to_string(),
            Err(err) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), "pid_value"),
                    format!("Failed to extract PID value as UTF-8: {}", err),
                ));
                return unknown_pid_header(node, source, "Could not decode @PID value");
            }
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "pid_header"),
            "Missing PID value in @PID header",
        ));
        return unknown_pid_header(node, source, "Missing PID value in @PID header");
    };

    Header::Pid {
        pid: PidValue::new(pid),
    }
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
