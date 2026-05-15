//! Line-level error routing between header and utterance analyzers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::{HEADER, LINE, UTTERANCE};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use crate::parser::tree_parsing::parser_helpers::{is_header, is_pre_begin_header};
use tree_sitter::Node;

use super::header::analyze_header_error;
use super::utterance::analyze_utterance_error;

/// Routes a line-scoped `ERROR` node to header or utterance analyzers.
pub(crate) fn analyze_line_error(
    error_node: Node,
    line_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) {
    let error_text = extract_utf8_text(error_node, source, errors, "line_error", "");
    let start = error_node.start_byte();
    let end = error_node.end_byte();

    if line_node.kind() == LINE {
        let mut cursor = line_node.walk();
        for child in line_node.children(&mut cursor) {
            let kind = child.kind();
            if kind == HEADER || is_header(kind) || is_pre_begin_header(kind) {
                analyze_header_error(error_node, line_node, source, errors);
                return;
            }
            if kind == UTTERANCE {
                analyze_utterance_error(error_node, line_node, source, errors);
                return;
            }
        }
    }

    // Fallback: if this looks like a header error
    if line_node.kind() == HEADER
        || is_header(line_node.kind())
        || is_pre_begin_header(line_node.kind())
    {
        analyze_header_error(error_node, line_node, source, errors);
        return;
    }

    if line_node.kind() == UTTERANCE {
        analyze_utterance_error(error_node, line_node, source, errors);
        return;
    }

    // Generic line error
    errors.report(ParseError::new(
        ErrorCode::UnparsableLine,
        Severity::Error,
        SourceLocation::from_offsets(start, end),
        ErrorContext::new(source, start..end, error_text),
        "Syntax error in line",
    ));
}
