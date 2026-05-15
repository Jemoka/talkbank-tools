//! Parsing for long-feature markers inside base content.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{LongFeatureBegin, LongFeatureEnd, LongFeatureLabel, UtteranceContent};
use crate::node_types::{
    AMPERSAND, LONG_FEATURE_BEGIN, LONG_FEATURE_BEGIN_MARKER, LONG_FEATURE_END,
    LONG_FEATURE_END_MARKER, LONG_FEATURE_LABEL,
};
use crate::parser::tree_parsing::parser_helpers::cst_assertions::{
    assert_child_count_exact, assert_child_kind_one_of, expect_child, expect_child_at,
};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse a long-feature indicator that marks the beginning or end of a scoped annotation and
/// emit the matching `UtteranceContent` marker for traceability.
///
/// CHAT long features (`&{l=...}` begin/end markers discussed in the Main Tier's Long Features
/// section) wrap multiple tokens with a named span so auxiliary analyses can refer to the
/// entire chunk. This parser validates only local CST structure (marker kind + label), captures
/// the label text, and attaches span metadata so downstream code can reproduce the original
/// `&{l=label` / `&}l=label` markers. Cross-token begin/end pairing semantics are enforced in
/// later validation layers, not in this structural parse step.
pub(crate) fn parse_long_feature(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    // Long feature - check if it's begin or end
    if !assert_child_count_exact(node, 1, source, errors, "long_feature") {
        return ParseOutcome::rejected();
    }

    if !assert_child_kind_one_of(
        node,
        0,
        &[LONG_FEATURE_BEGIN, LONG_FEATURE_END],
        source,
        errors,
        "long_feature",
    ) {
        return ParseOutcome::rejected();
    }

    let ParseOutcome::Parsed(feature_child) =
        expect_child_at(node, 0, source, errors, "long_feature")
    else {
        return ParseOutcome::rejected();
    };
    match feature_child.kind() {
        LONG_FEATURE_BEGIN => {
            if !assert_child_count_exact(feature_child, 3, source, errors, "long_feature_begin") {
                return ParseOutcome::rejected();
            }
            if expect_child(
                feature_child,
                0,
                AMPERSAND,
                source,
                errors,
                "long_feature_begin",
            )
            .is_none()
                || expect_child(
                    feature_child,
                    1,
                    LONG_FEATURE_BEGIN_MARKER,
                    source,
                    errors,
                    "long_feature_begin",
                )
                .is_none()
            {
                return ParseOutcome::rejected();
            }
            let ParseOutcome::Parsed(label_node) = expect_child(
                feature_child,
                2,
                LONG_FEATURE_LABEL,
                source,
                errors,
                "long_feature_begin",
            ) else {
                return ParseOutcome::rejected();
            };
            let label_text =
                extract_utf8_text(label_node, source, errors, "long_feature_begin_label", "");
            let span = Span::new(
                feature_child.start_byte() as u32,
                feature_child.end_byte() as u32,
            );
            let marker = LongFeatureBegin::new(LongFeatureLabel::new(label_text)).with_span(span);
            ParseOutcome::parsed(UtteranceContent::LongFeatureBegin(marker))
        }
        LONG_FEATURE_END => {
            if !assert_child_count_exact(feature_child, 3, source, errors, "long_feature_end") {
                return ParseOutcome::rejected();
            }
            if expect_child(
                feature_child,
                0,
                AMPERSAND,
                source,
                errors,
                "long_feature_end",
            )
            .is_none()
                || expect_child(
                    feature_child,
                    1,
                    LONG_FEATURE_END_MARKER,
                    source,
                    errors,
                    "long_feature_end",
                )
                .is_none()
            {
                return ParseOutcome::rejected();
            }
            let ParseOutcome::Parsed(label_node) = expect_child(
                feature_child,
                2,
                LONG_FEATURE_LABEL,
                source,
                errors,
                "long_feature_end",
            ) else {
                return ParseOutcome::rejected();
            };
            let label_text =
                extract_utf8_text(label_node, source, errors, "long_feature_end_label", "");
            let span = Span::new(
                feature_child.start_byte() as u32,
                feature_child.end_byte() as u32,
            );
            let marker = LongFeatureEnd::new(LongFeatureLabel::new(label_text)).with_span(span);
            ParseOutcome::parsed(UtteranceContent::LongFeatureEnd(marker))
        }
        _ => {
            if feature_child.is_error() {
                errors.report(ParseError::new(
                    ErrorCode::MalformedWordContent,
                    Severity::Error,
                    SourceLocation::from_offsets(
                        feature_child.start_byte(),
                        feature_child.end_byte(),
                    ),
                    ErrorContext::new(
                        source,
                        feature_child.start_byte()..feature_child.end_byte(),
                        "",
                    ),
                    format!(
                        "Malformed long_feature at byte {}..{}",
                        feature_child.start_byte(),
                        feature_child.end_byte()
                    ),
                ));
            }
            ParseOutcome::rejected()
        }
    }
}
