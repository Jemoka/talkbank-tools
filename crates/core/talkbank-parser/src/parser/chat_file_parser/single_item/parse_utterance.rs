//! Parse isolated utterances via whole-file recovery plus extraction.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::TreeSitterParser;
use super::helpers::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ParseError, ParseErrors, ParseResult, Severity,
    SourceLocation, SpanShift,
};
use crate::model::Line;
use crate::model::Utterance;

/// Parse one utterance fragment into `Utterance`.
///
/// This path intentionally reuses whole-file recovery so fragment parsing stays
/// aligned with normal utterance construction, including preceding headers and
/// attached dependent tiers.
pub(super) fn parse_utterance(parser: &TreeSitterParser, input: &str) -> ParseResult<Utterance> {
    let input_with_newline = if input.as_bytes().last().is_some_and(|b| *b == b'\n') {
        input.to_string()
    } else {
        format!("{}\n", input)
    };

    let is_full_chat = parser
        .parser
        .borrow_mut()
        .parse(&input_with_newline, None)
        .is_some_and(|tree| {
            let mut cursor = tree.root_node().walk();
            tree.root_node()
                .children(&mut cursor)
                .any(|child| child.kind() == crate::node_types::UTF8_HEADER)
        });

    let (to_parse, offset) = if is_full_chat {
        (input_with_newline, 0)
    } else {
        (
            format!("{}{}\n{}", MINIMAL_CHAT_PREFIX, input, MINIMAL_CHAT_SUFFIX),
            MINIMAL_CHAT_PREFIX.len(),
        )
    };

    let errors_sink = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(&to_parse, &errors_sink);
    let parse_errors = errors_sink.into_vec();
    if !parse_errors.is_empty() {
        return Err(ParseErrors::from(parse_errors));
    }

    let utterance = file
        .lines
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utterance) => Some(*utterance),
            _ => None,
        })
        .ok_or_else(|| {
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::MissingMainTier,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "No utterance found in parsed output",
            ));
            errors
        })?;

    let mut utterance = utterance;
    if offset > 0 {
        utterance.shift_spans_after(0, -(offset as i32));
    }

    Ok(utterance)
}
