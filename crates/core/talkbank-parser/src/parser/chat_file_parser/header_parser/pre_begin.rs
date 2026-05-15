//! Parsing for headers permitted before `@Begin`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Font_Header>

use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{self, Header, Line};
use crate::node_types::*;

use crate::parser::tree_parsing::header::parse_pid_header;

/// Build `Header::Unknown` from a malformed pre-begin header node.
fn unknown_header_from_node(
    node: tree_sitter::Node,
    input: &str,
    reason: impl Into<String>,
) -> Header {
    let text = match node.utf8_text(input.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => node.kind().to_string(),
    };

    Header::Unknown {
        text: model::WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: None,
    }
}

/// Parse and append one pre-`@Begin` header line.
pub fn handle_pre_begin_header(
    node: tree_sitter::Node,
    span: Span,
    input: &str,
    errors: &impl ErrorSink,
    lines: &mut Vec<Line>,
) {
    match node.kind() {
        PID_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_pid_header(node, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            lines.push(Line::header_with_span(header, span));
        }
        WINDOW_HEADER => {
            // Grammar: seq(prefix, header_sep, window_geometry, newline)
            // Child layout: [0]=prefix, [1]=sep, [2]=geometry, [3]=newline
            // Content at position 2
            let geometry = match node
                .child(2u32)
                .and_then(|child| child.utf8_text(input.as_bytes()).ok().map(str::to_string))
            {
                Some(value) => value,
                None => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                        ErrorContext::new(input, node.start_byte()..node.end_byte(), WINDOW_HEADER),
                        "Missing or invalid @Window geometry",
                    ));
                    lines.push(Line::header_with_span(
                        unknown_header_from_node(node, input, "Malformed @Window header"),
                        span,
                    ));
                    return;
                }
            };
            lines.push(Line::header_with_span(
                Header::Window {
                    geometry: model::WindowGeometry::new(geometry),
                },
                span,
            ));
        }
        COLOR_WORDS_HEADER => {
            // Grammar: seq(prefix, header_sep, color_word_list, newline)
            // Child layout: [0]=prefix, [1]=sep, [2]=color_word_list, [3]=newline
            // Content at position 2
            let colors = match node
                .child(2u32)
                .and_then(|child| child.utf8_text(input.as_bytes()).ok().map(str::to_string))
            {
                Some(value) => value,
                None => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                        ErrorContext::new(
                            input,
                            node.start_byte()..node.end_byte(),
                            COLOR_WORDS_HEADER,
                        ),
                        "Missing or invalid @Color words content",
                    ));
                    lines.push(Line::header_with_span(
                        unknown_header_from_node(node, input, "Malformed @Color words header"),
                        span,
                    ));
                    return;
                }
            };
            lines.push(Line::header_with_span(
                Header::ColorWords {
                    colors: model::ColorWordList::new(colors),
                },
                span,
            ));
        }
        FONT_HEADER => {
            // Grammar: seq(prefix, header_sep, font_spec, newline)
            // Child layout: [0]=prefix, [1]=sep, [2]=font_spec, [3]=newline
            // Content at position 2
            let font = match node
                .child(2u32)
                .and_then(|child| child.utf8_text(input.as_bytes()).ok().map(str::to_string))
            {
                Some(value) => value,
                None => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                        ErrorContext::new(input, node.start_byte()..node.end_byte(), FONT_HEADER),
                        "Missing or invalid @Font content",
                    ));
                    lines.push(Line::header_with_span(
                        unknown_header_from_node(node, input, "Malformed @Font header"),
                        span,
                    ));
                    return;
                }
            };
            lines.push(Line::header_with_span(
                Header::Font {
                    font: model::FontSpec::new(font),
                },
                span,
            ));
        }
        unknown => {
            errors.report(ParseError::new(
                ErrorCode::UnknownHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(input, node.start_byte()..node.end_byte(), unknown),
                format!(
                    "Unknown pre-begin header type '{}' - will be ignored",
                    unknown
                ),
            ));
        }
    }
}
