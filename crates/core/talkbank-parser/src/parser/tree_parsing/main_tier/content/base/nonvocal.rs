//! Parsing for nonvocal markers in base content.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{NonvocalBegin, NonvocalEnd, NonvocalLabel, NonvocalSimple, UtteranceContent};
use crate::node_types::{
    AMPERSAND, LONG_FEATURE_LABEL, NONVOCAL_BEGIN, NONVOCAL_BEGIN_MARKER, NONVOCAL_END,
    NONVOCAL_END_MARKER, NONVOCAL_SIMPLE, RIGHT_BRACE,
};
use crate::parser::tree_parsing::parser_helpers::cst_assertions::{
    assert_child_count_exact, assert_child_kind_one_of, expect_child, expect_child_at,
};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse nonvocal markers such as laughter, inhalations, or other tagged events into `UtteranceContent`.
///
/// Nonvocal markers use the `&` prefix inside the main tier and can either open a scoped
/// nonvocal span (`&{n=label`), close that scope (`&}n=label`), or appear as a single
/// self-contained token (`&{n=label}`).
/// We verify the exact child structure produced by the tree-sitter grammar and extract the
/// label text so that downstream features such as alignment and tooling can locate the
/// named nonvocal events with their spans. The parser also keeps a precise span so the
/// runtime can map the annotation back to the CHAT text described in the Main Tier appendix.
pub(crate) fn parse_nonvocal(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    // Nonvocal scope - check if it's begin, end, or simple
    if !assert_child_count_exact(node, 1, source, errors, "nonvocal") {
        return ParseOutcome::rejected();
    }

    if !assert_child_kind_one_of(
        node,
        0,
        &[NONVOCAL_BEGIN, NONVOCAL_END, NONVOCAL_SIMPLE],
        source,
        errors,
        "nonvocal",
    ) {
        return ParseOutcome::rejected();
    }

    let ParseOutcome::Parsed(nonvocal_child) = expect_child_at(node, 0, source, errors, "nonvocal")
    else {
        return ParseOutcome::rejected();
    };

    match nonvocal_child.kind() {
        NONVOCAL_BEGIN => {
            if !assert_child_count_exact(nonvocal_child, 3, source, errors, "nonvocal_begin") {
                return ParseOutcome::rejected();
            }
            if expect_child(
                nonvocal_child,
                0,
                AMPERSAND,
                source,
                errors,
                "nonvocal_begin",
            )
            .is_none()
                || expect_child(
                    nonvocal_child,
                    1,
                    NONVOCAL_BEGIN_MARKER,
                    source,
                    errors,
                    "nonvocal_begin",
                )
                .is_none()
            {
                return ParseOutcome::rejected();
            }
            let ParseOutcome::Parsed(label_node) = expect_child(
                nonvocal_child,
                2,
                LONG_FEATURE_LABEL,
                source,
                errors,
                "nonvocal_begin",
            ) else {
                return ParseOutcome::rejected();
            };
            let label_text =
                extract_utf8_text(label_node, source, errors, "nonvocal_begin_label", "");
            let span = Span::new(
                nonvocal_child.start_byte() as u32,
                nonvocal_child.end_byte() as u32,
            );
            let marker = NonvocalBegin::new(NonvocalLabel::new(label_text)).with_span(span);
            ParseOutcome::parsed(UtteranceContent::NonvocalBegin(marker))
        }
        NONVOCAL_END => {
            if !assert_child_count_exact(nonvocal_child, 3, source, errors, "nonvocal_end") {
                return ParseOutcome::rejected();
            }
            if expect_child(nonvocal_child, 0, AMPERSAND, source, errors, "nonvocal_end").is_none()
                || expect_child(
                    nonvocal_child,
                    1,
                    NONVOCAL_END_MARKER,
                    source,
                    errors,
                    "nonvocal_end",
                )
                .is_none()
            {
                return ParseOutcome::rejected();
            }
            let ParseOutcome::Parsed(label_node) = expect_child(
                nonvocal_child,
                2,
                LONG_FEATURE_LABEL,
                source,
                errors,
                "nonvocal_end",
            ) else {
                return ParseOutcome::rejected();
            };
            let label_text =
                extract_utf8_text(label_node, source, errors, "nonvocal_end_label", "");
            let span = Span::new(
                nonvocal_child.start_byte() as u32,
                nonvocal_child.end_byte() as u32,
            );
            let marker = NonvocalEnd::new(NonvocalLabel::new(label_text)).with_span(span);
            ParseOutcome::parsed(UtteranceContent::NonvocalEnd(marker))
        }
        NONVOCAL_SIMPLE => {
            if !assert_child_count_exact(nonvocal_child, 4, source, errors, "nonvocal_simple") {
                return ParseOutcome::rejected();
            }
            if expect_child(
                nonvocal_child,
                0,
                AMPERSAND,
                source,
                errors,
                "nonvocal_simple",
            )
            .is_none()
                || expect_child(
                    nonvocal_child,
                    1,
                    NONVOCAL_BEGIN_MARKER,
                    source,
                    errors,
                    "nonvocal_simple",
                )
                .is_none()
                || expect_child(
                    nonvocal_child,
                    3,
                    RIGHT_BRACE,
                    source,
                    errors,
                    "nonvocal_simple",
                )
                .is_none()
            {
                return ParseOutcome::rejected();
            }
            let ParseOutcome::Parsed(label_node) = expect_child(
                nonvocal_child,
                2,
                LONG_FEATURE_LABEL,
                source,
                errors,
                "nonvocal_simple",
            ) else {
                return ParseOutcome::rejected();
            };
            let label_text =
                extract_utf8_text(label_node, source, errors, "nonvocal_simple_label", "");
            let span = Span::new(
                nonvocal_child.start_byte() as u32,
                nonvocal_child.end_byte() as u32,
            );
            let marker = NonvocalSimple::new(NonvocalLabel::new(label_text)).with_span(span);
            ParseOutcome::parsed(UtteranceContent::NonvocalSimple(marker))
        }
        _ => {
            if nonvocal_child.is_error() {
                errors.report(ParseError::new(
                    ErrorCode::MalformedWordContent,
                    Severity::Error,
                    SourceLocation::from_offsets(
                        nonvocal_child.start_byte(),
                        nonvocal_child.end_byte(),
                    ),
                    ErrorContext::new(
                        source,
                        nonvocal_child.start_byte()..nonvocal_child.end_byte(),
                        "",
                    ),
                    format!(
                        "Malformed nonvocal at byte {}..{}",
                        nonvocal_child.start_byte(),
                        nonvocal_child.end_byte()
                    ),
                ));
            }
            ParseOutcome::rejected()
        }
    }
}
