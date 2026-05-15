//! Parsing for base (non-group) main-tier content items.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

mod internal_bullet;
mod long_feature;
mod nonvocal;
mod other_spoken;
mod overlap_point;

// Re-export overlap_point parser for use in other modules
pub(crate) use overlap_point::parse_overlap_point;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    BULLET, FREECODE, LONG_FEATURE, NONVOCAL, NONWORD_WITH_OPTIONAL_ANNOTATIONS,
    OTHER_SPOKEN_EVENT, OVERLAP_POINT, PAUSE_TOKEN, SEPARATOR, UNDERLINE_BEGIN, UNDERLINE_END,
    WORD_WITH_OPTIONAL_ANNOTATIONS,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::freecode::parse_freecode;
use super::nonword::parse_nonword_content;
use super::word::parse_word_content;
use crate::parser::tree_parsing::parser_helpers::{
    expect_child_at, parse_pause_node, parse_separator_node,
};

/// Parse one `base_content_item` into `UtteranceContent`.
///
/// The base content choices cover words, pauses, separators, typed nonwords, media bullets, overlap points,
/// underline markers, and scoped annotations (`&` long features) as described in the Main Tier section of the
/// CHAT manual. This function enforces that exactly one expected child exists, dispatches to the dedicated parser,
/// and rejects unexpected nodes so the parser mirrors the grammar in `grammar.js`.
pub(crate) fn parse_base_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();

    // Position 0: require exactly one child (one of the choice alternatives)
    if child_count == 0 {
        return ParseOutcome::rejected();
    }

    // CRITICAL: Use expect_child_at to check for MISSING nodes - prevents fake objects
    if let ParseOutcome::Parsed(child) = expect_child_at(node, 0u32, source, errors, "base_content")
    {
        let content = match child.kind() {
            WORD_WITH_OPTIONAL_ANNOTATIONS => parse_word_content(child, source, errors),
            PAUSE_TOKEN => {
                // Parse pause using node kind dispatch
                parse_pause_node(child, source, errors).map(UtteranceContent::Pause)
            }
            NONWORD_WITH_OPTIONAL_ANNOTATIONS => parse_nonword_content(child, source, errors),
            FREECODE => parse_freecode(child, source, errors),
            BULLET => internal_bullet::parse_internal_bullet(child, source, errors),
            OVERLAP_POINT => overlap_point::parse_overlap_point(child, source, errors),
            SEPARATOR => {
                // Parse separator using node kind dispatch
                parse_separator_node(child, source, errors).map(UtteranceContent::Separator)
            }
            UNDERLINE_BEGIN => {
                // Underline begin marker (\u0002\u0001)
                let span =
                    talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32);
                ParseOutcome::parsed(UtteranceContent::UnderlineBegin(
                    talkbank_model::UnderlineMarker::from_span(span),
                ))
            }
            UNDERLINE_END => {
                // Underline end marker (\u0002\u0002)
                let span =
                    talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32);
                ParseOutcome::parsed(UtteranceContent::UnderlineEnd(
                    talkbank_model::UnderlineMarker::from_span(span),
                ))
            }
            LONG_FEATURE => long_feature::parse_long_feature(child, source, errors),
            NONVOCAL => nonvocal::parse_nonvocal(child, source, errors),
            OTHER_SPOKEN_EVENT => other_spoken::parse_other_spoken_event(child, source, errors),
            _ => {
                // Truly unexpected/unrecognized base_content types - return explicit error
                errors.report(
                    ParseError::new(
                        ErrorCode::UnknownBaseContent,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                        format!("Unknown base content type '{}'", child.kind()),
                    )
                    .with_suggestion("This may be a new grammar feature not yet supported"),
                );
                ParseOutcome::rejected()
            }
        };

        // Check for unexpected extra children
        if child_count > 1 {
            for idx in 1..child_count {
                if let Some(extra) = node.child(idx as u32) {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(extra.start_byte(), extra.end_byte()),
                        ErrorContext::new(source, extra.start_byte()..extra.end_byte(), ""),
                        format!(
                            "Unexpected extra child '{}' at position {} of base_content",
                            extra.kind(),
                            idx
                        ),
                    ));
                }
            }
        }

        return content;
    }

    ParseOutcome::rejected()
}
