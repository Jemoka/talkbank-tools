//! Parsing for overlap-point tokens (`⌈`, `⌉`, `⌊`, `⌋` with optional indices).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::{OverlapIndex, OverlapPoint, OverlapPointKind, UtteranceContent};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Converts one overlap-point token node into `UtteranceContent`.
///
/// **Grammar Rule:**
/// ```text
/// overlap_point: $ => token(prec(10, choice(
///   seq('\u2308', optional(/[2-9]/)),  // ⌈ or ⌈2..⌈9
///   seq('\u2309', optional(/[2-9]/)),  // ⌉ or ⌉2..⌉9
///   seq('\u230A', optional(/[2-9]/)),  // ⌊ or ⌊2..⌊9
///   seq('\u230B', optional(/[2-9]/))   // ⌋ or ⌋2..⌋9
/// )))
/// ```
///
/// **Atomic token structure:**
/// - Overlap markers are single atomic tokens (e.g., "⌈", "⌊2", "⌉3")
/// - Index (2-9) is embedded in the token text, not a separate child node
/// - Parser extracts marker kind and optional index from token text
pub(crate) fn parse_overlap_point(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    // Extract text from atomic token
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(e) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("Failed to extract overlap point text: {}", e),
            ));
            return ParseOutcome::rejected();
        }
    };

    let mut chars = text.chars();

    // First character: overlap marker (⌈⌉⌊⌋)
    let marker = match chars.next() {
        Some(c) => c,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
                "Empty overlap point token".to_string(),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Map marker character to kind
    let kind = match marker {
        '\u{2308}' => OverlapPointKind::TopOverlapBegin, // ⌈
        '\u{2309}' => OverlapPointKind::TopOverlapEnd,   // ⌉
        '\u{230A}' => OverlapPointKind::BottomOverlapBegin, // ⌊
        '\u{230B}' => OverlapPointKind::BottomOverlapEnd, // ⌋
        other => {
            errors.report(ParseError::new(
                ErrorCode::UnbalancedOverlap,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
                format!(
                    "Unknown overlap point marker '{}' (U+{:04X})",
                    other, other as u32
                ),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Second character (optional): index digit 2-9
    let index = chars
        .next()
        .and_then(|digit_char| digit_char.to_digit(10).map(OverlapIndex::new));

    let span = crate::error::Span::new(node.start_byte() as u32, node.end_byte() as u32);
    ParseOutcome::parsed(UtteranceContent::OverlapPoint(
        OverlapPoint::new(kind, index).with_span(span),
    ))
}
