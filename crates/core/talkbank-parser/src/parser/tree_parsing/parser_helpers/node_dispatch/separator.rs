//! Separator-node parsing utilities.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::Separator;
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, FALLING_TO_LOW,
    FALLING_TO_MID, LEVEL_PITCH, NON_COLON_SEPARATOR, RISING_TO_HIGH, RISING_TO_MID, SEMICOLON,
    TAG_MARKER, UNMARKED_ENDING, UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a separator node into `Separator` using node-kind dispatch.
///
/// **Grammar Rule:**
/// ```text
/// separator: $ => choice(
///   seq(optional($.whitespaces), $.non_colon_separator),
///   seq($.whitespaces, $.colon),
/// ),
///
/// non_colon_separator: $ => choice(
///   $.comma, $.semicolon, $.tag_marker, $.vocative_marker,
///   $.ca_continuation_marker, $.unmarked_ending, $.uptake_symbol,
///   $.ca_no_break, $.ca_technical_break,
///   $.rising_to_high, $.rising_to_mid, $.level_pitch,
///   $.falling_to_mid, $.falling_to_low,
/// ),
/// ```
///
/// **Expected Structure:**
/// - Position 0: optional `whitespaces`
/// - Position 0 or 1: `non_colon_separator` OR `colon`
fn parse_non_colon_separator_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    let actual = if node.child_count() > 0 {
        node.child(0)
    } else {
        Some(node)
    };

    let Some(actual_sep) = actual else {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "non_colon_separator has no children".to_string(),
        ));
        return ParseOutcome::rejected();
    };

    match actual_sep.kind() {
        COMMA => ParseOutcome::parsed(Separator::Comma {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        SEMICOLON => ParseOutcome::parsed(Separator::Semicolon {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        TAG_MARKER => ParseOutcome::parsed(Separator::Tag {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        VOCATIVE_MARKER => ParseOutcome::parsed(Separator::Vocative {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        CA_CONTINUATION_MARKER => ParseOutcome::parsed(Separator::CaContinuation {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        UNMARKED_ENDING => ParseOutcome::parsed(Separator::UnmarkedEnding {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        UPTAKE_SYMBOL => ParseOutcome::parsed(Separator::Uptake {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        CA_NO_BREAK => ParseOutcome::parsed(Separator::CaNoBreak {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        CA_TECHNICAL_BREAK => ParseOutcome::parsed(Separator::CaTechnicalBreak {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        RISING_TO_HIGH => ParseOutcome::parsed(Separator::RisingToHigh {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        RISING_TO_MID => ParseOutcome::parsed(Separator::RisingToMid {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        LEVEL_PITCH => ParseOutcome::parsed(Separator::Level {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        FALLING_TO_MID => ParseOutcome::parsed(Separator::FallingToMid {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        FALLING_TO_LOW => ParseOutcome::parsed(Separator::FallingToLow {
            span: Span::new(actual_sep.start_byte() as u32, actual_sep.end_byte() as u32),
        }),
        _ => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(actual_sep.start_byte(), actual_sep.end_byte()),
                ErrorContext::new(source, actual_sep.start_byte()..actual_sep.end_byte(), ""),
                format!("Unknown non_colon_separator kind '{}'", actual_sep.kind()),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Parse a full `separator` CST node including optional leading whitespace.
pub(crate) fn parse_separator_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    let child_count = node.child_count();
    let mut idx = 0;

    // Skip optional leading whitespace
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        idx += 1;
    }

    // Now we should have either non_colon_separator or colon
    if idx < child_count
        && let Some(sep_child) = node.child(idx as u32)
    {
        match sep_child.kind() {
            NON_COLON_SEPARATOR => {
                return parse_non_colon_separator_node(sep_child, source, errors);
            }
            COLON => {
                return ParseOutcome::parsed(Separator::Colon {
                    span: Span::new(sep_child.start_byte() as u32, sep_child.end_byte() as u32),
                });
            }
            _ => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(sep_child.start_byte(), sep_child.end_byte()),
                    ErrorContext::new(source, sep_child.start_byte()..sep_child.end_byte(), ""),
                    format!(
                        "Expected 'non_colon_separator' or 'colon' in separator, found '{}'",
                        sep_child.kind()
                    ),
                ));
                return ParseOutcome::rejected();
            }
        }
    }

    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        "Separator node has no separator content after whitespace".to_string(),
    ));

    ParseOutcome::rejected()
}

/// Parse either `separator` or separator-like leaf nodes.
pub(crate) fn parse_separator_like(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Separator> {
    match node.kind() {
        crate::node_types::SEPARATOR => {
            if node.child_count() == 0
                && let Ok(text) = node.utf8_text(source.as_bytes())
            {
                return match text {
                    ":" => ParseOutcome::parsed(Separator::Colon {
                        span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
                    }),
                    "," => ParseOutcome::parsed(Separator::Comma {
                        span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
                    }),
                    ";" => ParseOutcome::parsed(Separator::Semicolon {
                        span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
                    }),
                    _ => ParseOutcome::rejected(),
                };
            }
            parse_separator_node(node, source, errors)
        }
        crate::node_types::NON_COLON_SEPARATOR => {
            parse_non_colon_separator_node(node, source, errors)
        }
        crate::node_types::COLON => ParseOutcome::parsed(Separator::Colon {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::COMMA => ParseOutcome::parsed(Separator::Comma {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::SEMICOLON => ParseOutcome::parsed(Separator::Semicolon {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::TAG_MARKER => ParseOutcome::parsed(Separator::Tag {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::VOCATIVE_MARKER => ParseOutcome::parsed(Separator::Vocative {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::CA_CONTINUATION_MARKER => {
            ParseOutcome::parsed(Separator::CaContinuation {
                span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
            })
        }
        crate::node_types::UNMARKED_ENDING => ParseOutcome::parsed(Separator::UnmarkedEnding {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::UPTAKE_SYMBOL => ParseOutcome::parsed(Separator::Uptake {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::CA_NO_BREAK => ParseOutcome::parsed(Separator::CaNoBreak {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::CA_TECHNICAL_BREAK => {
            ParseOutcome::parsed(Separator::CaTechnicalBreak {
                span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
            })
        }
        crate::node_types::RISING_TO_HIGH => ParseOutcome::parsed(Separator::RisingToHigh {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::RISING_TO_MID => ParseOutcome::parsed(Separator::RisingToMid {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::LEVEL_PITCH => ParseOutcome::parsed(Separator::Level {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::FALLING_TO_MID => ParseOutcome::parsed(Separator::FallingToMid {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        crate::node_types::FALLING_TO_LOW => ParseOutcome::parsed(Separator::FallingToLow {
            span: Span::new(node.start_byte() as u32, node.end_byte() as u32),
        }),
        _ => ParseOutcome::rejected(),
    }
}
