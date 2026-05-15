//! Parser for main-tier freecode spans (`[^ ... ]`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Freecodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{Freecode, UtteranceContent};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse freecode node [^ text] into UtteranceContent.
///
/// **Grammar Rule (coarsened):**
/// ```text
/// freecode: $ => token(/\[\^ [^\]\r\n]+\]/)
/// ```
///
/// The node is now a single leaf token. Extract the code by stripping
/// the `[^ ` prefix and `]` suffix, then trimming trailing whitespace.
pub fn parse_freecode(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::EmptyUtterance,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "freecode"),
                format!("UTF-8 decoding error in freecode: {err}"),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Strip "[^ " prefix and "]" suffix
    let code = text
        .strip_prefix("[^ ")
        .and_then(|s| s.strip_suffix(']'))
        .map(|s| s.trim_end());

    match code {
        Some(c) if !c.is_empty() => {
            let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
            let freecode = Freecode::with_span(c, span);
            ParseOutcome::parsed(UtteranceContent::Freecode(freecode))
        }
        _ => {
            errors.report(ParseError::new(
                ErrorCode::EmptyUtterance,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "freecode"),
                "Empty freecode content",
            ));
            ParseOutcome::rejected()
        }
    }
}
