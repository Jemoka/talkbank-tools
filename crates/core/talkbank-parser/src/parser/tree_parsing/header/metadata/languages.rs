//! Parsing for `@Languages` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>

use crate::node_types::*;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::model::{Header, LanguageCode, WarningText};

/// Build `Header::Unknown` for malformed `@Languages` input.
fn unknown_languages_header(node: Node, source: &str, parse_reason: impl Into<String>) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Languages".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Languages:\t<code>[, <code>...]".to_string()),
    }
}

/// Parse Languages header from tree-sitter node
///
/// **Grammar Rule**:
/// ```javascript
/// languages_header: $ => seq(
///     token('@Languages:\t'),
///     $.languages_contents,
///     $.newline
/// )
///
/// languages_contents: $ => seq(
///     $.language_code,
///     repeat(seq(',', $.whitespace, $.language_code))
/// )
/// ```
pub fn parse_languages_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    let mut codes = Vec::new();

    // Verify this is a languages_header node
    if node.kind() != LANGUAGES_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected languages_header node, got: {}", node.kind()),
        ));
        return unknown_languages_header(
            node,
            source,
            "Languages header CST node had unexpected kind",
        );
    }

    // Find languages_contents child (prefix + header_sep + contents + newline)
    let contents = match find_child_by_kind(node, LANGUAGES_CONTENTS) {
        Some(child) => child,
        _ => {
            errors.report(ParseError::new(
                ErrorCode::EmptyLanguagesHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    source,
                    node.start_byte()..node.end_byte(),
                    "languages_header",
                ),
                "Missing languages_contents in @Languages header",
            ));
            return unknown_languages_header(
                node,
                source,
                "Missing languages_contents in @Languages header",
            );
        }
    };

    // Extract language_code children
    let child_count = contents.child_count();
    let mut idx = 0;

    // First language code (required)
    if idx < child_count
        && let Some(child) = contents.child(idx as u32)
    {
        if child.kind() == LANGUAGE_CODE {
            if let Ok(code) = child.utf8_text(source.as_bytes()) {
                codes.push(LanguageCode::new(code));
            }
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::EmptyLanguagesHeader,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(
                    source,
                    child.start_byte()..child.end_byte(),
                    "languages_contents",
                ),
                format!(
                    "Expected 'language_code' at position {}, got: {}",
                    idx,
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Subsequent language codes (optional)
    while idx < child_count {
        // Skip optional whitespace before comma (grammar allows this)
        while idx < child_count {
            if let Some(child) = contents.child(idx as u32) {
                if child.kind() == WHITESPACES {
                    idx += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Check for comma
        if let Some(child) = contents.child(idx as u32) {
            if child.kind() == COMMA {
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::EmptyLanguagesHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "languages_contents",
                    ),
                    format!("Expected ',' at position {}, got: {}", idx, child.kind()),
                ));
                idx += 1;
                continue;
            }
        } else {
            break;
        }

        // Check for whitespace
        if let Some(child) = contents.child(idx as u32) {
            if child.kind() == WHITESPACES {
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::EmptyLanguagesHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "languages_contents",
                    ),
                    format!(
                        "Expected 'whitespace' at position {}, got: {}",
                        idx,
                        child.kind()
                    ),
                ));
                idx += 1;
            }
        }

        // Check for language_code
        if let Some(child) = contents.child(idx as u32) {
            if child.kind() == LANGUAGE_CODE {
                if let Ok(code) = child.utf8_text(source.as_bytes()) {
                    codes.push(LanguageCode::new(code));
                }
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::EmptyLanguagesHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "languages_contents",
                    ),
                    format!(
                        "Expected 'language_code' at position {}, got: {}",
                        idx,
                        child.kind()
                    ),
                ));
                idx += 1;
            }
        }
    }

    Header::Languages {
        codes: codes.into(),
    }
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
