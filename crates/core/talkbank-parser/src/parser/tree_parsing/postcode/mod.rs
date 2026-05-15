//! Parser for main-tier postcodes (`[+ ... ]`).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::Postcode;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a single postcode node [+ text].
///
/// **Grammar Rule (coarsened):**
/// ```text
/// postcode: $ => token(/\[\+ [^\]\r\n]+\]/)
/// ```
///
/// The node is now a single leaf token. Extract the code by stripping
/// the `[+ ` prefix and `]` suffix, then trimming trailing whitespace.
pub fn parse_postcode_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Postcode> {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::InvalidPostcode,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "postcode"),
                format!("UTF-8 decoding error in postcode: {err}"),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Strip "[+ " prefix and "]" suffix
    let code = text
        .strip_prefix("[+ ")
        .and_then(|s| s.strip_suffix(']'))
        .map(|s| s.trim_end());

    match code {
        Some(c) if !c.is_empty() => ParseOutcome::parsed(Postcode::new(c)),
        _ => {
            errors.report(ParseError::new(
                ErrorCode::InvalidPostcode,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "postcode"),
                "Postcode is missing required content".to_string(),
            ));
            ParseOutcome::rejected()
        }
    }
}
