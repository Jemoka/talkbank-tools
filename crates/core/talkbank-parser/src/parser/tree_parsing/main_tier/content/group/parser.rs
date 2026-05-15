//! Parsing for annotated angle-bracket groups (`< ... >[...]`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Annotations>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{Annotated, BracketedContent, Group, Retrace, UtteranceContent};
use crate::node_types::{BASE_ANNOTATIONS, CONTENTS, GREATER_THAN, LESS_THAN, WHITESPACES};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::annotations::parse_scoped_annotations;
use super::contents::parse_group_contents;

/// Parse one `group_with_annotations` node into group utterance content, preserving the `<...>[...]` semantics.
///
/// CHAT group annotations appear as `< contents > base_annotations` and are described in the Group and Annotation
/// sections of the manual. This parser consumes the expected `<`, optional whitespace, contents block,
/// optional trailing whitespace, closing `>`, and the required annotations block, emitting either a
/// bare `Group` or an `AnnotatedGroup` depending on whether scoped annotations exist. Any deviation from that
/// structure is reported through `ParseError` so users can correlate the diagnostic with the manual’s grammar.
pub(crate) fn parse_group_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();
    // Pre-allocate: group typically has 1-5 items, annotations typically 0-2
    let mut group_items = Vec::with_capacity(4);
    let mut annotations = Vec::with_capacity(2);
    let mut retrace_kind = None;
    let mut idx = 0;

    // Grammar: group_with_annotations: $ => seq(
    //   $.less_than,
    //   optional($.whitespaces),  // Allow leading whitespace after <
    //   $.contents,
    //   optional($.whitespaces),  // Allow trailing whitespace before >
    //   $.greater_than,
    //   $.base_annotations  // REQUIRED
    // )

    // Position 0: '<' (required)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == LESS_THAN {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected '<' at start of group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Position 1: optional whitespaces after <
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        // Skip optional whitespace
        idx += 1;
    }

    // Next: contents (required)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == CONTENTS {
            group_items = parse_group_contents(child, source, errors);
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected 'contents' in group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Next: optional whitespaces before >
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
        && child.kind() == WHITESPACES
    {
        // Skip optional whitespace
        idx += 1;
    }

    // Next: '>' (required)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == GREATER_THAN {
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected '>' in group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Next: base_annotations (required for groups)
    if idx < child_count
        && let Some(child) = node.child(idx as u32)
    {
        if child.kind() == BASE_ANNOTATIONS {
            let parsed = parse_scoped_annotations(child, source, errors);
            annotations = parsed.content;
            retrace_kind = parsed.retrace;
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Expected 'base_annotations' in group_with_annotations, found '{}'",
                    child.kind()
                ),
            ));
            idx += 1;
        }
    }

    // Check for unexpected extra children
    if idx < child_count {
        for extra_idx in idx..child_count {
            if let Some(extra) = node.child(extra_idx as u32) {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(extra.start_byte(), extra.end_byte()),
                    ErrorContext::new(source, extra.start_byte()..extra.end_byte(), ""),
                    format!(
                        "Unexpected extra child '{}' at position {} of group_with_annotations",
                        extra.kind(),
                        extra_idx
                    ),
                ));
            }
        }
    }

    if group_items.is_empty() {
        return ParseOutcome::rejected();
    }

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bracketed = BracketedContent::new(group_items);
    let group = Group::new(bracketed).with_span(span);

    if let Some(kind) = retrace_kind {
        // Group retrace: <content> [/] etc.
        let retrace = Retrace::new(group.content, kind)
            .as_group()
            .with_annotations(annotations)
            .with_span(span);
        ParseOutcome::parsed(UtteranceContent::Retrace(Box::new(retrace)))
    } else if annotations.is_empty() {
        ParseOutcome::parsed(UtteranceContent::Group(group))
    } else {
        let annotated = Annotated::new(group)
            .with_scoped_annotations(annotations)
            .with_span(span);
        ParseOutcome::parsed(UtteranceContent::AnnotatedGroup(annotated))
    }
}
