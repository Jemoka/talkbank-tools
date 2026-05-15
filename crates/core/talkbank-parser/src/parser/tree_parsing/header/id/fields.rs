//! Field-level extraction helpers for `@ID` header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use crate::node_types::ID_SEX;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::ParseOutcome;
use talkbank_model::model::Sex;

use super::helpers::{extract_text_with_errors, get_child_or_report, skip_whitespace};

/// Parse a required text field and advance the `@ID` cursor.
pub(super) fn parse_required_text_field(
    id_contents: Node,
    idx: &mut usize,
    child_count: usize,
    source: &str,
    errors: &impl ErrorSink,
    error_code: ErrorCode,
    error_message: &str,
) -> ParseOutcome<String> {
    skip_whitespace(id_contents, idx, child_count);
    if *idx < child_count {
        let child = get_child_or_report(id_contents, *idx as u32, source, errors, "id_contents");
        let text = extract_text_with_errors(child, source, errors, "id_contents");
        *idx += 1; // field
        skip_whitespace(id_contents, idx, child_count);
        *idx += 1; // separator
        text
    } else {
        errors.report(ParseError::new(
            error_code,
            Severity::Error,
            SourceLocation::from_offsets(id_contents.start_byte(), id_contents.end_byte()),
            ErrorContext::new(
                source,
                id_contents.start_byte()..id_contents.end_byte(),
                "id_contents",
            ),
            error_message,
        ));
        ParseOutcome::rejected()
    }
}

/// Parse an optional text field and advance the `@ID` cursor.
///
/// Skips leading/trailing whitespace nodes produced by the grammar's
/// `optional($.whitespaces)` wrappers around optional fields.
pub(super) fn parse_optional_text_field(
    id_contents: Node,
    idx: &mut usize,
    child_count: usize,
    source: &str,
    errors: &impl ErrorSink,
    expected_kind: &str,
) -> ParseOutcome<Option<String>> {
    skip_whitespace(id_contents, idx, child_count);
    if *idx < child_count {
        let child = get_child_or_report(id_contents, *idx as u32, source, errors, "id_contents");
        let text = match child {
            ParseOutcome::Parsed(c) => {
                if c.kind() == expected_kind {
                    *idx += 1;
                    extract_text_with_errors(ParseOutcome::parsed(c), source, errors, "id_contents")
                        .map(Some)
                } else {
                    // Kind mismatch - this is optional, so treat as absent.
                    ParseOutcome::parsed(None)
                }
            }
            ParseOutcome::Rejected => ParseOutcome::rejected(),
        };
        skip_whitespace(id_contents, idx, child_count);
        *idx += 1; // separator
        text
    } else {
        ParseOutcome::parsed(None)
    }
}

/// Parse optional sex field (`male`/`female`/generic) and advance the `@ID` cursor.
///
/// The grammar captures known values (`male`, `female`) and unknown values
/// via `generic_id_sex`. All are wrapped in an `id_sex` node; text is passed
/// to `Sex::from_text()` which classifies or returns `Unsupported`.
pub(super) fn parse_optional_sex_field(
    id_contents: Node,
    idx: &mut usize,
    child_count: usize,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<Sex>> {
    skip_whitespace(id_contents, idx, child_count);
    if *idx < child_count {
        let child = get_child_or_report(id_contents, *idx as u32, source, errors, "id_contents");
        let result = match child {
            ParseOutcome::Parsed(c) => {
                if c.kind() == ID_SEX {
                    *idx += 1;
                    match extract_text_with_errors(
                        ParseOutcome::parsed(c),
                        source,
                        errors,
                        "id_contents",
                    ) {
                        ParseOutcome::Parsed(text) => {
                            ParseOutcome::parsed(Some(Sex::from_text(&text)))
                        }
                        ParseOutcome::Rejected => ParseOutcome::rejected(),
                    }
                } else {
                    // Kind mismatch - optional field is absent.
                    ParseOutcome::parsed(None)
                }
            }
            ParseOutcome::Rejected => ParseOutcome::rejected(),
        };
        skip_whitespace(id_contents, idx, child_count);
        *idx += 1; // separator
        result
    } else {
        ParseOutcome::parsed(None)
    }
}

/// Parse optional final field without consuming a trailing separator.
pub(super) fn parse_optional_terminal_field(
    id_contents: Node,
    idx: usize,
    child_count: usize,
    source: &str,
    errors: &impl ErrorSink,
    expected_kind: &str,
) -> ParseOutcome<Option<String>> {
    let mut local_idx = idx;
    skip_whitespace(id_contents, &mut local_idx, child_count);
    if local_idx < child_count {
        let child =
            get_child_or_report(id_contents, local_idx as u32, source, errors, "id_contents");
        match child {
            ParseOutcome::Parsed(c) => {
                if c.kind() == expected_kind {
                    extract_text_with_errors(ParseOutcome::parsed(c), source, errors, "id_contents")
                        .map(Some)
                } else {
                    // Kind mismatch - optional terminal field is absent.
                    ParseOutcome::parsed(None)
                }
            }
            ParseOutcome::Rejected => ParseOutcome::rejected(),
        }
    } else {
        ParseOutcome::parsed(None)
    }
}
