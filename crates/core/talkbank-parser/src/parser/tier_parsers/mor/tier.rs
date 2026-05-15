//! Tier-level `%mor` parsing.
//!
//! This file parses one morphology tier line, then delegates each item to
//! `parse_mor_content` and records any terminator token carried in the tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::node_types as kind;
use talkbank_model::ParseOutcome;
use talkbank_model::model::content::Terminator;
use talkbank_model::model::{Mor, MorTier, MorTierType};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

use super::item::parse_mor_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_child_at, is_terminator,
};

/// Converts `%mor` tier content into a `MorTier`.
///
/// **Grammar Rule:**
/// ```text
/// mor_dependent_tier: $ => seq(
///     $.mor_tier_prefix,   // Position 0
///     $.tier_sep,          // Position 1
///     $.mor_contents,      // Position 2
///     $.newline            // Position 3
/// )
/// ```
pub fn parse_mor_tier_inner(
    node: Node,
    source: &str,
    tier_type: MorTierType,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    // Position 2: mor_contents (required by grammar)
    let mor_contents = match expect_child_at(node, 2, source, errors, "mor_dependent_tier") {
        ParseOutcome::Parsed(contents) => contents,
        ParseOutcome::Rejected => return ParseOutcome::Rejected,
    };

    let child_count = mor_contents.child_count();
    let mut items: Vec<Mor> = Vec::with_capacity(child_count / 2 + 1);
    // Local typed terminator slot accumulated as we walk children.
    // If still None at end-of-loop, reject the whole tier so cross-
    // tier validation surfaces a typed parse-failure rather than a
    // silent count-mismatch downstream.
    let mut terminator: Option<Terminator> = None;
    // Per-item parse failures reject the whole tier (instead of
    // silently dropping the item, which would leave the tier
    // miscounted and the failure invisible to cross-tier validators).
    let mut had_item_failure = false;
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = mor_contents.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes before processing
            if !check_not_missing(child, source, errors, "mor_contents") {
                had_item_failure = true;
                idx += 1;
                continue;
            }

            let kind = child.kind();
            match kind {
                kind::MOR_CONTENT => match parse_mor_content(child, source, errors) {
                    ParseOutcome::Parsed(mor) => items.push(mor),
                    ParseOutcome::Rejected => {
                        // Diagnostic already streamed via ErrorSink.
                        had_item_failure = true;
                    }
                },
                _ if is_terminator(kind) || kind == kind::TERMINATOR => {
                    // Lift the terminator's raw bytes into the typed
                    // `Terminator` enum at parse time.
                    match child.utf8_text(source.as_bytes()) {
                        Ok(text) if !text.is_empty() => {
                            match Terminator::try_from_chat_str(text.trim()) {
                                Some(typed) => {
                                    terminator = Some(typed);
                                }
                                None => {
                                    errors.report(ParseError::new(
                                        ErrorCode::TreeParsingError,
                                        Severity::Error,
                                        SourceLocation::from_offsets(
                                            child.start_byte(),
                                            child.end_byte(),
                                        ),
                                        ErrorContext::new(
                                            source,
                                            child.start_byte()..child.end_byte(),
                                            "mor_terminator",
                                        ),
                                        format!("Unrecognized %mor terminator string {text:?}"),
                                    ));
                                    return ParseOutcome::Rejected;
                                }
                            }
                        }
                        Ok(_) => {
                            errors.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                                ErrorContext::new(
                                    source,
                                    child.start_byte()..child.end_byte(),
                                    "mor_terminator",
                                ),
                                "Empty terminator node in %mor contents".to_string(),
                            ));
                            return ParseOutcome::Rejected;
                        }
                        Err(err) => {
                            errors.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                                ErrorContext::new(
                                    source,
                                    child.start_byte()..child.end_byte(),
                                    "mor_terminator",
                                ),
                                format!("UTF-8 decoding error in %mor terminator: {err}"),
                            ));
                            return ParseOutcome::Rejected;
                        }
                    }
                }
                kind::WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(child, source, "mor_contents"));
                    had_item_failure = true;
                }
            }
        }
        idx += 1;
    }

    if had_item_failure {
        return ParseOutcome::Rejected;
    }

    let Some(typed_terminator) = terminator else {
        errors.report(ParseError::new(
            ErrorCode::MissingTerminator,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(
                source,
                node.start_byte()..node.end_byte(),
                "mor_dependent_tier",
            ),
            "%mor tier is missing a terminator".to_string(),
        ));
        return ParseOutcome::Rejected;
    };

    ParseOutcome::Parsed(MorTier::new(tier_type, items, typed_terminator).with_span(span))
}
