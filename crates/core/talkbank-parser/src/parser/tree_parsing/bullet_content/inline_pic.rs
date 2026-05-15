//! Parser for inline picture references (`%pic`) inside tier text.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Extract filename from inline_pic text.
///
/// The inline_pic node is now a single token matching:
///   `\u{15}%pic:"FILENAME"\u{15}`
///
/// This helper strips the delimiters and returns the filename.
fn parse_inline_pic_text(text: &str) -> Option<&str> {
    // Strip \u{15}%pic:" prefix and "\u{15} suffix
    let inner = text.strip_prefix("\u{15}%pic:\"")?;
    let filename = inner.strip_suffix("\"\u{15}")?;
    if filename.is_empty() {
        None
    } else {
        Some(filename)
    }
}

/// Converts one `inline_pic` node into a picture filename.
///
/// **Grammar Rule (coarsened):**
/// ```text
/// inline_pic: $ => token(/\u0015%pic:"[a-zA-Z0-9][a-zA-Z0-9\/\-_'.]*"\u0015/)
/// ```
///
/// **Returns:** Option<String> (errors streamed via ErrorSink)
/// - Some(filename) if valid
/// - None if parsing failed
pub(super) fn parse_inline_pic(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<String> {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "inline_pic"),
                format!("UTF-8 decoding error in inline_pic: {err}"),
            ));
            return ParseOutcome::rejected();
        }
    };

    match parse_inline_pic_text(text) {
        Some(filename) => ParseOutcome::parsed(filename.to_string()),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "inline_pic"),
                "Missing filename in inline picture reference".to_string(),
            ));
            ParseOutcome::rejected()
        }
    }
}
