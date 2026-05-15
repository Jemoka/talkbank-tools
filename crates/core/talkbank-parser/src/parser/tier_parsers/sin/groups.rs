//! Group extraction helpers for `%sin` content.
//!
//! `%sin` allows both flat gesture tokens and bracketed grouped spans.
//! These helpers normalize CST nodes into `SinItem` sequences.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>

use crate::node_types::{SIN_BEGIN_GROUP, SIN_GROUPED_CONTENT, SIN_WORD, WHITESPACES};
use talkbank_model::model::{SinGroupGestures, SinItem, SinToken};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;

/// Extracts `SinItem` values from a `sin_group` node.
pub(super) fn extract_sin_group_items(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinItem> {
    if let Some(first_child) = node.child(0u32) {
        match first_child.kind() {
            SIN_WORD => {
                let text = extract_utf8_text(first_child, source, errors, "sin_word", "");
                if !text.is_empty() {
                    vec![SinItem::Token(SinToken::new_unchecked(text))]
                } else {
                    vec![]
                }
            }
            SIN_BEGIN_GROUP => {
                if let Some(grouped_content) = node.child(1u32) {
                    if grouped_content.kind() == SIN_GROUPED_CONTENT {
                        let gestures =
                            extract_sin_grouped_content_tokens(grouped_content, source, errors);
                        if !gestures.is_empty() {
                            vec![SinItem::SinGroup(SinGroupGestures::new(gestures))]
                        } else {
                            vec![]
                        }
                    } else {
                        errors.report(ParseError::new(
                            ErrorCode::TreeParsingError,
                            Severity::Error,
                            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                            ErrorContext::new(
                                source,
                                node.start_byte()..node.end_byte(),
                                grouped_content.kind(),
                            ),
                            format!(
                                "Expected sin_grouped_content in grouped sin, got: {}",
                                grouped_content.kind()
                            ),
                        ));
                        vec![]
                    }
                } else {
                    vec![]
                }
            }
            _ => {
                let text = extract_utf8_text(node, source, errors, "sin_item", "");
                if !text.is_empty() {
                    vec![SinItem::Token(SinToken::new_unchecked(text))]
                } else {
                    vec![]
                }
            }
        }
    } else {
        vec![]
    }
}

/// Extracts `SinToken` values from grouped `%sin` content.
fn extract_sin_grouped_content_tokens(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinToken> {
    let mut tokens = Vec::new();
    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                SIN_WORD => {
                    let text = extract_utf8_text(child, source, errors, "sin_word", "");
                    if !text.is_empty() {
                        tokens.push(SinToken::new_unchecked(text));
                    }
                }
                WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(child, source, "sin_grouped_content"));
                }
            }
        }
        idx += 1;
    }

    tokens
}
