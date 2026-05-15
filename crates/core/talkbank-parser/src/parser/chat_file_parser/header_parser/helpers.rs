//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::{self, ChatOptionFlag};
use crate::node_types::{CONTINUATION, OPTION_NAME, OPTIONS_CONTENTS, REST_OF_LINE};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse `@Options` flag tokens into typed `ChatOptionFlag` values.
///
/// All values are accepted including unrecognized ones (stored as
/// `ChatOptionFlag::Unsupported`). The validator flags unsupported values.
pub(super) fn parse_options_flags(
    header_actual: tree_sitter::Node,
    input: &str,
    _errors: &impl ErrorSink,
) -> Vec<ChatOptionFlag> {
    // Grammar: seq(prefix, header_sep, options_contents, newline)
    let mut flags = Vec::new();
    if let Some(option_list_node) = find_child_by_kind(header_actual, OPTIONS_CONTENTS) {
        let mut cursor = option_list_node.walk();
        for child in option_list_node.children(&mut cursor) {
            if child.kind() == OPTION_NAME
                && let Ok(text) = child.utf8_text(input.as_bytes())
            {
                if text.is_empty() {
                    // Empty option_name comes from grammar recovery for "@Options:\t".
                    // Represent as empty options list and let validation report E533.
                    continue;
                }
                // All values are accepted; unsupported ones are flagged by the validator.
                flags.push(ChatOptionFlag::from_text(text));
            }
        }
    }
    flags
}

/// Extract required text content from a child node kind.
pub(super) fn get_required_content_by_kind(
    node: Node,
    input: &str,
    kind: &str,
    errors: &impl ErrorSink,
    header_kind: &str,
) -> ParseOutcome<String> {
    if let Some(child) = find_child_by_kind(node, kind) {
        match child.utf8_text(input.as_bytes()) {
            Ok(text) => ParseOutcome::parsed(text.to_string()),
            Err(err) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(input, child.start_byte()..child.end_byte(), header_kind),
                    format!("Failed to extract UTF-8 text for {}: {}", header_kind, err),
                ));
                ParseOutcome::rejected()
            }
        }
    } else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(input, node.start_byte()..node.end_byte(), header_kind),
            format!("Missing expected {} node in {}", kind, header_kind),
        ));
        ParseOutcome::rejected()
    }
}

/// Find first direct child matching `kind`.
pub(super) fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

/// Parse optional label text used by `@Bg`, `@Eg`, and `@G` headers.
pub(crate) fn parse_optional_gem_label(
    node: Option<Node>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let Some(node) = node else {
        return ParseOutcome::parsed(None);
    };
    let mut cursor = node.walk();
    let mut label = String::new();
    let mut saw_text = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            REST_OF_LINE => {
                if let Ok(text) = child.utf8_text(input.as_bytes())
                    && !text.is_empty()
                {
                    label.push_str(text);
                    saw_text = true;
                }
            }
            CONTINUATION => {
                if saw_text {
                    label.push(' ');
                }
            }
            _ => errors.report(unexpected_node_error(child, input, "gem label")),
        }
    }

    if label.is_empty() {
        ParseOutcome::parsed(None)
    } else {
        ParseOutcome::parsed(Some(model::GemLabel::new(label)))
    }
}
