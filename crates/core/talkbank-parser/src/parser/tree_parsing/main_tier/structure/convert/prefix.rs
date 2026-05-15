//! Prefix-field extraction for `main_tier` conversion.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::node_types::{COLON, SPEAKER, STAR, TAB};
use tree_sitter::Node;

use super::{PrefixData, report_cst_access_failure, report_missing_child, report_unexpected_child};

/// Parse `*`, speaker code, colon, and tab from `main_tier` children.
pub(super) fn parse_prefix(
    node: Node,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> PrefixData {
    let mut speaker: Option<String> = None;
    let mut speaker_span = Span::DUMMY;
    let child_count = node.child_count();
    let mut idx = 0;

    // Position 0: star
    if idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            if child.kind() == STAR {
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::StructuralOrderError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!(
                        "Expected 'star' (*) at position 0 of main_tier, found '{}'",
                        child.kind()
                    ),
                ));
                idx += 1;
            }
        } else {
            report_cst_access_failure(node, source, errors, idx);
            idx += 1;
        }
    } else {
        report_missing_child(
            original_input,
            errors,
            ErrorCode::MissingSpeaker,
            "Missing star (*) at beginning of main tier",
        );
    }

    // Position 1: speaker
    if idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            if child.kind() == SPEAKER {
                if child.is_missing() || child.start_byte() == child.end_byte() {
                    errors.report(
                        ParseError::new(
                            ErrorCode::MissingSpeaker,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                            "Missing speaker in main tier",
                        )
                        .with_suggestion("Main tier should start with *SPEAKER:"),
                    );
                } else {
                    speaker = Some(source[child.start_byte()..child.end_byte()].to_string());
                    speaker_span = Span::new(child.start_byte() as u32, child.end_byte() as u32);
                }
                idx += 1;
            } else {
                report_unexpected_child(child, source, errors, "speaker", idx);
                idx += 1;
            }
        } else {
            report_cst_access_failure(node, source, errors, idx);
            idx += 1;
        }
    } else {
        report_missing_child(
            original_input,
            errors,
            ErrorCode::MissingSpeaker,
            "Missing speaker in main tier",
        );
    }

    // Position 2: colon
    if idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            if child.kind() == COLON {
                if child.start_byte() == child.end_byte() {
                    errors.report(
                        ParseError::new(
                            ErrorCode::EmptyColon,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(
                                original_input,
                                0..original_input.len(),
                                original_input,
                            ),
                            "Empty colon (zero-width node) in main tier".to_string(),
                        )
                        .with_suggestion("Add ':' after speaker code (e.g., '*CHI:')"),
                    );
                }
                idx += 1;
            } else {
                report_unexpected_child(child, source, errors, "colon", idx);
                idx += 1;
            }
        } else {
            report_cst_access_failure(node, source, errors, idx);
            idx += 1;
        }
    } else {
        report_missing_child(
            original_input,
            errors,
            ErrorCode::MissingColonAfterSpeaker,
            "Missing colon (:) after speaker in main tier",
        );
    }

    // Position 3: tab
    if idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            if child.kind() == TAB {
                idx += 1;
            } else {
                report_unexpected_child(child, source, errors, "tab", idx);
                idx += 1;
            }
        } else {
            report_cst_access_failure(node, source, errors, idx);
            idx += 1;
        }
    } else {
        report_missing_child(
            original_input,
            errors,
            ErrorCode::StructuralOrderError,
            "Missing tab after colon in main tier",
        );
    }

    PrefixData {
        speaker,
        speaker_span,
        idx,
    }
}
