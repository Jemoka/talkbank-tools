//! Parser for pause tokens (`(.)`, `(..)`, `(3.5)`, etc.).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{Pause, PauseDuration, PauseTimedDuration};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a pause node to Pause enum using token text dispatch.
///
/// After coarsening, `pause_token` is a single atomic leaf token that matches
/// `(.)`, `(..)`, `(...)`, or timed patterns like `(3.5)` / `(3:2.5)`.
/// We dispatch on the token text to determine the pause type.
pub(crate) fn parse_pause_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Pause> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("UTF-8 decoding error in pause_token: {err}"),
            ));
            return ParseOutcome::rejected();
        }
    };

    match text {
        "(.)" => ParseOutcome::parsed(Pause::new(PauseDuration::Short).with_span(span)),
        "(..)" => ParseOutcome::parsed(Pause::new(PauseDuration::Medium).with_span(span)),
        "(...)" => ParseOutcome::parsed(Pause::new(PauseDuration::Long).with_span(span)),
        _ => {
            // Timed pause: strip surrounding parens and extract duration text
            let duration_text = text
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(text);

            if duration_text.is_empty() {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), duration_text),
                    "pause_token duration is empty",
                ));
                return ParseOutcome::rejected();
            }

            ParseOutcome::parsed(
                Pause::new(PauseDuration::Timed(PauseTimedDuration::new(
                    duration_text.to_string(),
                )))
                .with_span(span),
            )
        }
    }
}
