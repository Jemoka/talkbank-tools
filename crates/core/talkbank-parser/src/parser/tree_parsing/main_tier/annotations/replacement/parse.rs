//! Parsing for replacement annotations (`[: ... ]`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::Replacement;
use crate::node_types::{RIGHT_BRACKET, STANDALONE_WORD, WHITESPACES};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::helpers::{expect_child_kind, report_unexpected_kind};
use crate::parser::tree_parsing::main_tier::word::convert_word_node;

/// Parse a replacement annotation node into `Replacement`.
pub(crate) fn parse_replacement(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Replacement> {
    let child_count = node.child_count();
    let mut words = Vec::with_capacity((child_count / 2).max(1));
    let mut idx = 0;

    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        // Report error if not the expected kind, but always advance
        expect_child_kind(
            node,
            source,
            child,
            "left_bracket",
            "replacement",
            idx,
            errors,
        );
        idx += 1;
    }

    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        // Report error if not the expected kind, but always advance
        expect_child_kind(node, source, child, "colon", "replacement", idx, errors);
        idx += 1;
    }

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                WHITESPACES => {
                    idx += 1;
                    continue;
                }
                RIGHT_BRACKET => break,
                STANDALONE_WORD => {
                    // CRITICAL: Check for MISSING nodes - tree-sitter error recovery
                    // can insert placeholder nodes that still have the expected kind
                    if child.is_missing() {
                        errors.report(ParseError::new(
                            ErrorCode::ReplacementParseError,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                            format!(
                                "Missing word in replacement at position {} (tree-sitter inserted placeholder)",
                                idx
                            ),
                        ));
                        idx += 1;
                        continue;
                    }

                    // Check for zero-width (empty) word nodes - these have no actual content
                    if child.start_byte() == child.end_byte() {
                        errors.report(ParseError::new(
                            ErrorCode::ReplacementParseError,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                            "Replacement text empty".to_string(),
                        ));
                        idx += 1;
                        continue;
                    }

                    if let ParseOutcome::Parsed(word) = convert_word_node(child, source, errors) {
                        words.push(word);
                    }
                    idx += 1;
                    continue;
                }
                _ => {
                    report_unexpected_kind(
                        node,
                        source,
                        child,
                        "replacement",
                        format!(
                            "Expected 'standalone_word' or ']' at position {}, found '{}'",
                            idx,
                            child.kind()
                        ),
                        errors,
                    );
                    idx += 1;
                    continue;
                }
            }
        } else {
            break;
        }
    }

    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() != RIGHT_BRACKET
    {
        errors.report(ParseError::new(
            ErrorCode::ReplacementParseError,
            Severity::Error,
            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
            format!(
                "Expected ']' at end of replacement, found '{}'",
                child.kind()
            ),
        ));
    }

    if words.is_empty() {
        ParseOutcome::rejected()
    } else {
        ParseOutcome::parsed(Replacement::new(words))
    }
}
