//! Header-context error analysis for malformed header lines.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::{
    DATE_HEADER, HEADER, ID_HEADER, LANGUAGES_HEADER, MEDIA_HEADER, PARTICIPANTS_HEADER,
};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use tree_sitter::Node;

/// Classifies an `ERROR` node while treating the line as a header line.
pub(crate) fn analyze_header_error(
    error_node: Node,
    line_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) {
    let error_text = extract_utf8_text(error_node, source, errors, "header_error", "");
    let start = error_node.start_byte();
    let end = error_node.end_byte();

    let mut header_kind: Option<&str> = None;
    let mut cursor = line_node.walk();
    for child in line_node.children(&mut cursor) {
        let kind = child.kind();
        if kind == HEADER {
            if let Some(inner) = child.child(0u32) {
                header_kind = Some(inner.kind());
                break;
            }
        } else {
            header_kind = Some(kind);
            break;
        }
    }

    if let Some(kind) = header_kind {
        match kind {
            PARTICIPANTS_HEADER => {
                errors.report(ParseError::new(
                    ErrorCode::EmptyParticipantsHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, ""),
                    "@Participants header cannot be empty",
                ));
                return;
            }
            LANGUAGES_HEADER => {
                errors.report(ParseError::new(
                    ErrorCode::EmptyLanguagesHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, ""),
                    "@Languages header cannot be empty",
                ));
                return;
            }
            DATE_HEADER => {
                errors.report(ParseError::new(
                    ErrorCode::EmptyDateHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, ""),
                    "@Date header cannot be empty",
                ));
                return;
            }
            MEDIA_HEADER => {
                errors.report(ParseError::new(
                    ErrorCode::EmptyMediaHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, ""),
                    "@Media header cannot be empty",
                ));
                return;
            }
            ID_HEADER => {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidIDFormat,
                        Severity::Error,
                        SourceLocation::from_offsets(start, end),
                        ErrorContext::new(source, start..end, error_text),
                        "Invalid @ID header format: structure could not be parsed",
                    )
                    .with_suggestion(
                        "@ID requires exactly 10 pipe-separated fields: @ID:\tlang|corpus|speaker|age|sex|group|SES|role|education|custom|",
                    ),
                );
                return;
            }
            _ => {}
        }
    }

    // Generic header error
    errors.report(ParseError::new(
        ErrorCode::UnparsableHeader,
        Severity::Error,
        SourceLocation::from_offsets(start, end),
        ErrorContext::new(source, start..end, error_text),
        "Syntax error in header",
    ));
}
