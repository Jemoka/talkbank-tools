//! Parsing for `@Media` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>

use crate::node_types::*;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Header, MediaHeader, MediaStatus, MediaType, WarningText};

/// Build `Header::Unknown` for malformed `@Media` input.
fn unknown_media_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Media".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Media:\tfilename, audio|video[, status]".to_string()),
    }
}

/// Decode UTF-8 child text for media header fields.
fn decode_child_text(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) -> ParseOutcome<String> {
    match child.utf8_text(source.as_bytes()) {
        Ok(text) => ParseOutcome::parsed(text.to_string()),
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), context),
                format!("Failed to extract UTF-8 text from {}: {}", context, err),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Parse Media header from tree-sitter node
///
/// **Grammar Rule:**
/// ```javascript
/// media_header: $ => seq(
///     '@Media:\t',         // Position 0 (combined token)
///     $.media_contents,    // Position 1
///     $.newline           // Position 2
/// )
///
/// media_contents: $ => seq(
///     $.media_filename,  // Position 0
///     ',',               // Position 1
///     $.whitespaces,     // Position 2
///     $.media_type,      // Position 3
///     ...
/// )
/// ```
pub fn parse_media_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a media_header node
    if node.kind() != MEDIA_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected media_header node, got: {}", node.kind()),
        ));
        return unknown_media_header(node, source, "Media header CST node had unexpected kind");
    }

    // Extract media_contents (prefix + header_sep + contents + newline)
    let contents = match find_child_by_kind(node, MEDIA_CONTENTS) {
        Some(child) => child,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "media_header"),
                "Missing media_contents in @Media header",
            ));
            return unknown_media_header(node, source, "Missing media_contents in @Media header");
        }
    };

    // Extract filename from position 0
    let filename = if let Some(child) = contents.child(0u32) {
        if child.kind() == MEDIA_FILENAME {
            match decode_child_text(child, source, errors, "media_filename") {
                ParseOutcome::Parsed(text) => text,
                ParseOutcome::Rejected => {
                    return unknown_media_header(node, source, "Could not decode @Media filename");
                }
            }
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(
                    source,
                    child.start_byte()..child.end_byte(),
                    "media_filename",
                ),
                format!(
                    "Expected media_filename node at @Media content position 0, got {}",
                    child.kind()
                ),
            ));
            return unknown_media_header(node, source, "Missing media filename in @Media header");
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(contents.start_byte(), contents.end_byte()),
            ErrorContext::new(
                source,
                contents.start_byte()..contents.end_byte(),
                MEDIA_CONTENTS,
            ),
            "Missing media_filename node in @Media header",
        ));
        return unknown_media_header(node, source, "Missing media filename in @Media header");
    };

    // Extract media_type from position 3 (after comma and whitespace).
    // All values accepted via from_text(); unsupported ones flagged by the validator.
    let media_type = if let Some(child) = contents.child(3u32) {
        if child.kind() == MEDIA_TYPE {
            let ParseOutcome::Parsed(type_text) =
                decode_child_text(child, source, errors, "media_type")
            else {
                return unknown_media_header(node, source, "Could not decode @Media type");
            };
            MediaType::from_text(&type_text)
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), "media_type"),
                format!(
                    "Expected media_type node at @Media content position 3, got {}",
                    child.kind()
                ),
            ));
            return unknown_media_header(node, source, "Missing media type in @Media header");
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(contents.start_byte(), contents.end_byte()),
            ErrorContext::new(
                source,
                contents.start_byte()..contents.end_byte(),
                MEDIA_CONTENTS,
            ),
            "Missing media_type node in @Media header",
        ));
        return unknown_media_header(node, source, "Missing media type in @Media header");
    };

    // Extract optional status from position 6 (after second comma and whitespace at positions 4 and 5).
    // All values accepted via from_text(); unsupported ones flagged by the validator.
    let status = if let Some(child) = contents.child(6) {
        if child.kind() == MEDIA_STATUS {
            let ParseOutcome::Parsed(status_text) =
                decode_child_text(child, source, errors, "media_status")
            else {
                return unknown_media_header(node, source, "Could not decode @Media status");
            };
            Some(MediaStatus::from_text(&status_text))
        } else {
            None
        }
    } else {
        None
    };

    let mut media_header = MediaHeader::new(filename, media_type);
    if let Some(s) = status {
        media_header = media_header.with_status(s);
    }
    Header::Media(media_header)
}

/// Finds child by kind.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
