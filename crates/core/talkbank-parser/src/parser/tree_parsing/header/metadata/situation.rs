//! Parsing for `@Situation` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>

use crate::node_types::{FREE_TEXT, SITUATION_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::model::{Header, SituationDescription, WarningText};

/// Build `Header::Unknown` for malformed `@Situation` input.
fn unknown_situation_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Situation".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Situation:\t<description>".to_string()),
    }
}

/// Parse Situation header from tree-sitter node
///
/// **Grammar Rule:**
/// ```javascript
/// situation_header: $ => seq(
///     '@', 'Situation', $.colon, $.tab,
///     $.free_text,    // Position 4
///     $.newline
/// )
/// ```
pub fn parse_situation_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a situation_header node
    if node.kind() != SITUATION_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected situation_header node, got: {}", node.kind()),
        ));
        return unknown_situation_header(
            node,
            source,
            "Situation header CST node had unexpected kind",
        );
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
                        "situation_text",
                    ),
                    format!("Failed to extract @Situation text as UTF-8: {}", err),
                ));
                return unknown_situation_header(node, source, "Could not decode @Situation text");
            }
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                source,
                node.start_byte()..node.end_byte(),
                "situation_header",
            ),
            "Missing situation text in @Situation header",
        ));
        return unknown_situation_header(
            node,
            source,
            "Missing situation text in @Situation header",
        );
    };

    Header::Situation {
        text: SituationDescription::new(text),
    }
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
