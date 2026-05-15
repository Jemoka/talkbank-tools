//! Parsing of utterance-end tails (terminator + postcodes + media bullet).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorVec, ParseError, Severity, SourceLocation,
};
use crate::model::{Bullet, Postcode, Terminator};
use crate::node_types::{BULLET, FINAL_CODES, NEWLINE, POSTCODE, SPACE, TERMINATOR, WHITESPACES};
use talkbank_model::Span;
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;

use crate::parser::tree_parsing::postcode::parse_postcode_node;
use talkbank_model::ParseOutcome;

use super::terminator::terminator_from_node_kind;

/// Result type for utterance_end parsing: ((terminator, postcodes, bullet), errors).
type UtteranceEndResult = (
    (Option<Terminator>, Vec<Postcode>, Option<Bullet>),
    ErrorVec,
);

/// Extracts terminator, postcodes, and optional trailing media bullet.
pub fn parse_utterance_end(node: Node, source: &str) -> UtteranceEndResult {
    let mut terminator: Option<Terminator> = None;
    let mut postcodes: Vec<Postcode> = Vec::new();
    let mut bullet: Option<Bullet> = None;
    let mut errors: ErrorVec = ErrorVec::new();

    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            let kind = child.kind();

            if let Some(term) = terminator_from_node_kind(
                kind,
                Span::new(child.start_byte() as u32, child.end_byte() as u32),
            ) {
                terminator = Some(term);
                idx += 1;
                continue;
            }

            match kind {
                TERMINATOR => {
                    if let Some(term_child) = child.child(0u32) {
                        let term_kind = term_child.kind();
                        if let Some(term) = terminator_from_node_kind(
                            term_kind,
                            Span::new(term_child.start_byte() as u32, term_child.end_byte() as u32),
                        ) {
                            terminator = Some(term);
                        } else {
                            errors.push(ParseError::new(
                                ErrorCode::InvalidPostcode,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    term_child.start_byte(),
                                    term_child.end_byte(),
                                ),
                                ErrorContext::new(
                                    source,
                                    term_child.start_byte()..term_child.end_byte(),
                                    "",
                                ),
                                format!("Unknown terminator type '{}'", term_kind),
                            ));
                        }
                    } else {
                        errors.push(ParseError::new(
                            ErrorCode::MissingTerminator,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                            "Terminator node has no child",
                        ));
                    }
                }
                FINAL_CODES => {
                    let final_count = child.child_count();
                    for final_idx in 0..final_count {
                        if let Some(final_child) = child.child(final_idx as u32)
                            && final_child.kind() == POSTCODE
                        {
                            let postcode_errors = ErrorCollector::new();
                            if let ParseOutcome::Parsed(postcode) =
                                parse_postcode_node(final_child, source, &postcode_errors)
                            {
                                postcodes.push(postcode);
                            }
                            errors.extend(postcode_errors.into_vec());
                        }
                    }
                }
                WHITESPACES | SPACE | NEWLINE => {}
                BULLET => {
                    match crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps(
                        child, source,
                    ) {
                        Some((start_ms, end_ms)) => {
                            let span =
                                Span::new(child.start_byte() as u32, child.end_byte() as u32);
                            bullet = Some(Bullet::new(start_ms, end_ms).with_span(span));
                        }
                        None => {
                            // `parse_bullet_node_timestamps` returns None
                            // when the bullet subtree carries a tree-sitter
                            // ERROR (e.g. the deprecated `·\d+_\d+-·`
                            // skip marker that the grammar rejects) or
                            // when a named timestamp field is missing /
                            // unparseable. Silently dropping the bullet
                            // was the old behavior; now we surface E360
                            // so the file does not pass validation with
                            // a malformed timestamp.
                            let bullet_text =
                                child.utf8_text(source.as_bytes()).unwrap_or("<unreadable>");
                            errors.push(ParseError::new(
                                ErrorCode::InvalidMediaBullet,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    child.start_byte(),
                                    child.end_byte(),
                                ),
                                ErrorContext::new(
                                    source,
                                    child.start_byte()..child.end_byte(),
                                    bullet_text,
                                ),
                                format!(
                                    "Invalid media bullet: grammar rejected '{}'. Legal form: ·START_END· with numeric timestamps only",
                                    bullet_text
                                ),
                            ));
                        }
                    }
                }
                _ => {
                    errors.push(unexpected_node_error(child, source, "utterance_end"));
                }
            }
            idx += 1;
        } else {
            break;
        }
    }

    ((terminator, postcodes, bullet), errors)
}
