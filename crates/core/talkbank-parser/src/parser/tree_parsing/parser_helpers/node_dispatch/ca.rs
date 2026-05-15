//! Parsers for Conversation Analysis marker tokens.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Unicode_Option>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{CADelimiter, CADelimiterType, CAElement, CAElementType};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Map a single Unicode character to a CA element type.
fn ca_element_type_from_char(ch: char) -> Option<CAElementType> {
    match ch {
        '\u{2260}' => Some(CAElementType::BlockedSegments), // ≠
        '\u{223E}' => Some(CAElementType::Constriction),    // ∾
        '\u{2051}' => Some(CAElementType::Hardening),       // ⁑
        '\u{2907}' => Some(CAElementType::HurriedStart),    // ⤇
        '\u{2219}' => Some(CAElementType::Inhalation),      // ∙
        '\u{1F29}' => Some(CAElementType::LaughInWord),     // Ἡ
        '\u{2193}' => Some(CAElementType::PitchDown),       // ↓
        '\u{21BB}' => Some(CAElementType::PitchReset),      // ↻
        '\u{2191}' => Some(CAElementType::PitchUp),         // ↑
        '\u{2906}' => Some(CAElementType::SuddenStop),      // ⤆
        _ => None,
    }
}

/// Map a single Unicode character to a CA delimiter type.
fn ca_delimiter_type_from_char(ch: char) -> Option<CADelimiterType> {
    match ch {
        '\u{2206}' => Some(CADelimiterType::Faster),       // ∆
        '\u{2207}' => Some(CADelimiterType::Slower),       // ∇
        '\u{00B0}' => Some(CADelimiterType::Softer),       // °
        '\u{2581}' => Some(CADelimiterType::LowPitch),     // ▁
        '\u{2594}' => Some(CADelimiterType::HighPitch),    // ▔
        '\u{263A}' => Some(CADelimiterType::SmileVoice),   // ☺
        '\u{264B}' => Some(CADelimiterType::BreathyVoice), // ♋
        '\u{2047}' => Some(CADelimiterType::Unsure),       // ⁇
        '\u{222C}' => Some(CADelimiterType::Whisper),      // ∬
        '\u{03AB}' => Some(CADelimiterType::Yawn),         // Ϋ
        '\u{222E}' => Some(CADelimiterType::Singing),      // ∮
        '\u{21AB}' => Some(CADelimiterType::SegmentRepetition), // ↫
        '\u{204E}' => Some(CADelimiterType::Creaky),       // ⁎
        '\u{25C9}' => Some(CADelimiterType::Louder),       // ◉
        '\u{00A7}' => Some(CADelimiterType::Precise),      // §
        _ => None,
    }
}

/// Converts one `ca_element` token node to `CAElement`.
///
/// After coarsening, ca_element is a single-character token.
/// We dispatch by examining the token text character.
pub(crate) fn parse_ca_element_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<CAElement> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let text = extract_utf8_text(node, source, errors, "ca_element", "");

    let Some(ch) = text.chars().next() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Empty CA element token",
        ));
        return ParseOutcome::rejected();
    };

    match ca_element_type_from_char(ch) {
        Some(element_type) => ParseOutcome::parsed(CAElement::new(element_type).with_span(span)),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("Unknown CA element character '{ch}'"),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Converts one `ca_delimiter` token node to `CADelimiter`.
///
/// After coarsening, ca_delimiter is a single-character token.
/// We dispatch by examining the token text character.
pub(crate) fn parse_ca_delimiter_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<CADelimiter> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let text = extract_utf8_text(node, source, errors, "ca_delimiter", "");

    let Some(ch) = text.chars().next() else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Empty CA delimiter token",
        ));
        return ParseOutcome::rejected();
    };

    match ca_delimiter_type_from_char(ch) {
        Some(delimiter_type) => {
            ParseOutcome::parsed(CADelimiter::new(delimiter_type).with_span(span))
        }
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("Unknown CA delimiter character '{ch}'"),
            ));
            ParseOutcome::rejected()
        }
    }
}
