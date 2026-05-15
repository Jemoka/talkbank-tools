//! Parsing for `@Types` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use crate::node_types::{TYPES_ACTIVITY, TYPES_DESIGN, TYPES_GROUP, TYPES_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, TypesHeader, WarningText};

/// Build `Header::Unknown` for malformed `@Types` input.
fn unknown_types_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Types".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Types:\tdesign, activity, group".to_string()),
    }
}

/// Parse Types header from tree-sitter node
///
/// **Grammar Rule:**
/// ```javascript
/// types_header: $ => seq(
///     '@', 'Types', $.colon, $.tab,
///     $.types_design,      // Position 4: design type (cross, long, observ)
///     $.comma,             // Position 5
///     $.whitespaces,       // Position 6
///     $.types_activity,    // Position 7: activity type (toyplay, narrative, etc.)
///     $.comma,             // Position 8
///     $.whitespaces,       // Position 9
///     $.types_group,       // Position 10: group type (TD, SLI, ASD, etc.)
///     $.newline            // Position 11
/// )
/// ```
///
/// The @Types header has three mandatory fields: design, activity, group.
pub fn parse_types_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a types_header node
    if node.kind() != TYPES_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected types_header node, got: {}", node.kind()),
        ));
        return unknown_types_header(node, source, "Types header CST node had unexpected kind");
    }

    // Grammar: seq(prefix, header_sep, types_design, comma, whitespaces, types_activity, comma, whitespaces, types_group, newline)
    let ParseOutcome::Parsed(design) =
        find_child_text(node, source, errors, TYPES_DESIGN, "types_design")
    else {
        return unknown_types_header(node, source, "Missing design field in @Types header");
    };

    let ParseOutcome::Parsed(activity) =
        find_child_text(node, source, errors, TYPES_ACTIVITY, "types_activity")
    else {
        return unknown_types_header(node, source, "Missing activity field in @Types header");
    };

    let ParseOutcome::Parsed(group) =
        find_child_text(node, source, errors, TYPES_GROUP, "types_group")
    else {
        return unknown_types_header(node, source, "Missing group field in @Types header");
    };

    let types_header = TypesHeader::new(design, activity, group);

    Header::Types(types_header)
}

/// Find first child of `kind` and extract its UTF-8 text.
fn find_child_text(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    kind: &str,
    label: &str,
) -> ParseOutcome<String> {
    let mut cursor = node.walk();
    let Some(child) = node
        .children(&mut cursor)
        .find(|child| child.kind() == kind)
    else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), "types_header"),
            format!("Missing {} in @Types header", label),
        ));
        return ParseOutcome::rejected();
    };

    match child.utf8_text(source.as_bytes()) {
        Ok(text) => ParseOutcome::parsed(text.to_string()),
        Err(e) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), label),
                format!("Failed to extract UTF-8 text from {}: {}", label, e),
            ));
            ParseOutcome::rejected()
        }
    }
}
