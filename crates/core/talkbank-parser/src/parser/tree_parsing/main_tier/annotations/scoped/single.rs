//! Parses one scoped annotation token (or wrapper) at a time.
//!
//! This module is the main dispatch point from coarsened tree-sitter nodes
//! into typed `ContentAnnotation` values.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::ContentAnnotation;
use crate::node_types::{
    ALT_ANNOTATION, BASE_ANNOTATION, ERROR_MARKER_ANNOTATION, EXCLUDE_MARKER,
    EXPLANATION_ANNOTATION, INDEXED_OVERLAP_FOLLOWS, INDEXED_OVERLAP_PRECEDES, PARA_ANNOTATION,
    PERCENT_ANNOTATION, RETRACE_COMPLETE, RETRACE_MULTIPLE, RETRACE_PARTIAL, RETRACE_REFORMULATION,
    SCOPED_CONTRASTIVE_STRESSING, SCOPED_STRESSING, SCOPED_UNCERTAIN,
};
use crate::tokens;
use talkbank_model::ParseOutcome;
use talkbank_model::model::RetraceKind;
use tree_sitter::Node;

/// Result of parsing a single annotation token: either a content annotation
/// or a retrace marker.
pub(crate) enum ParsedAnnotation {
    /// A non-retrace content annotation (`[*]`, `[= text]`, `[!]`, etc.)
    Content(ContentAnnotation),
    /// A retrace marker (`[/]`, `[//]`, `[///]`, `[/-]`)
    Retrace(RetraceKind),
}

/// Converts one annotation node like `[*]`, `[= text]`, or `[<2]`.
///
/// After Phase 5 coarsening, most annotations are atomic tokens. Parsing is
/// delegated to [`crate::tokens`] which provides the canonical
/// parse functions for each coarsened token type.
pub(crate) fn parse_single_annotation(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    let node_kind = node.kind();

    // With supertypes, we may receive either:
    // 1. A `base_annotation` wrapper node (look at child(0) for the concrete type)
    // 2. A concrete annotation type directly (process node itself)
    let annotation_node = if node_kind == BASE_ANNOTATION {
        let child_count = node.child_count();
        if child_count == 0 {
            errors.report(ParseError::new(
                ErrorCode::ContentAnnotationParseError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                "base_annotation has no children (expected exactly one annotation type)",
            ));
            return ParseOutcome::rejected();
        }
        match node.child(0u32) {
            Some(child) => child,
            None => return ParseOutcome::rejected(),
        }
    } else {
        node
    };

    let raw = &source[annotation_node.start_byte()..annotation_node.end_byte()];

    match annotation_node.kind() {
        // Text-bearing annotations — delegate to tokens API
        EXPLANATION_ANNOTATION => delegate_content_or_error(
            tokens::parse_explanation_token(raw),
            annotation_node,
            source,
            errors,
        ),
        PARA_ANNOTATION => delegate_content_or_error(
            tokens::parse_para_token(raw),
            annotation_node,
            source,
            errors,
        ),
        ALT_ANNOTATION => delegate_content_or_error(
            tokens::parse_alt_token(raw),
            annotation_node,
            source,
            errors,
        ),
        PERCENT_ANNOTATION => delegate_content_or_error(
            tokens::parse_percent_token(raw),
            annotation_node,
            source,
            errors,
        ),
        ERROR_MARKER_ANNOTATION => delegate_content_or_error(
            tokens::parse_error_marker_token(raw),
            annotation_node,
            source,
            errors,
        ),
        // Overlap annotations — delegate to tokens API
        INDEXED_OVERLAP_PRECEDES => delegate_content_or_error(
            tokens::parse_overlap_precedes_token(raw),
            annotation_node,
            source,
            errors,
        ),
        INDEXED_OVERLAP_FOLLOWS => delegate_content_or_error(
            tokens::parse_overlap_follows_token(raw),
            annotation_node,
            source,
            errors,
        ),
        // Scoped symbols — already atomic tokens, no payload
        SCOPED_STRESSING => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Stressing))
        }
        SCOPED_CONTRASTIVE_STRESSING => ParseOutcome::parsed(ParsedAnnotation::Content(
            ContentAnnotation::ContrastiveStressing,
        )),
        SCOPED_UNCERTAIN => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Uncertain))
        }
        // Retrace markers — parsed as RetraceKind, not ContentAnnotation
        RETRACE_COMPLETE => ParseOutcome::parsed(ParsedAnnotation::Retrace(RetraceKind::Full)),
        RETRACE_PARTIAL => ParseOutcome::parsed(ParsedAnnotation::Retrace(RetraceKind::Partial)),
        RETRACE_MULTIPLE => ParseOutcome::parsed(ParsedAnnotation::Retrace(RetraceKind::Multiple)),
        RETRACE_REFORMULATION => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace(RetraceKind::Reformulation))
        }
        // Exclude marker — already atomic token
        EXCLUDE_MARKER => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Exclude))
        }
        _ => {
            errors.report(ParseError::new(
                ErrorCode::ContentAnnotationParseError,
                Severity::Error,
                SourceLocation::from_offsets(
                    annotation_node.start_byte(),
                    annotation_node.end_byte(),
                ),
                ErrorContext::new(
                    source,
                    annotation_node.start_byte()..annotation_node.end_byte(),
                    annotation_node.kind(),
                ),
                format!("Unknown base annotation type '{}'", annotation_node.kind()),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Convert a content annotation token parse result into a ParseOutcome, reporting an error on failure.
fn delegate_content_or_error(
    result: Option<ContentAnnotation>,
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    match result {
        Some(annotation) => ParseOutcome::parsed(ParsedAnnotation::Content(annotation)),
        None => {
            errors.report(ParseError::new(
                ErrorCode::ContentAnnotationParseError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!(
                    "Failed to parse {} token: '{}'",
                    node.kind(),
                    &source[node.start_byte()..node.end_byte()]
                ),
            ));
            ParseOutcome::rejected()
        }
    }
}
