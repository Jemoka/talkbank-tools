//! Parsing for `@T` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Thumbnail_Header>

use crate::node_types::{FREE_TEXT, T_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::model::{Header, TDescription, WarningText};

/// Build `Header::Unknown` for malformed `@T` input.
fn unknown_t_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@T".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @T:\t<description>".to_string()),
    }
}

/// Parse @T header
///
/// **Grammar Rule:**
/// ```text
/// t_header: $ => seq(token('@T:\t'), $.free_text, $.newline)
/// ```
pub fn parse_t_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a t_header node
    if node.kind() != T_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected t_header node, got: {}", node.kind()),
        ));
        return unknown_t_header(node, source, "T header CST node had unexpected kind");
    }

    // Grammar: seq(prefix, header_sep, free_text, newline)
    let text = if let Some(child) = find_child_by_kind(node, FREE_TEXT) {
        match child.utf8_text(source.as_bytes()) {
            Ok(text) => text.to_string(),
            Err(err) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "t_header_text",
                    ),
                    format!("Failed to extract @T text as UTF-8: {}", err),
                ));
                return unknown_t_header(node, source, "Could not decode @T text");
            }
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "t_header"),
            "Missing text in @T header",
        ));
        return unknown_t_header(node, source, "Missing text in @T header");
    };

    Header::T {
        text: TDescription::new(text),
    }
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
