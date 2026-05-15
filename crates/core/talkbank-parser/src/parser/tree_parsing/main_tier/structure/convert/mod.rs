//! Convert `main_tier` CST nodes into `MainTier` model values.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{MainTier, Postcode, Terminator, UtteranceContent};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::content::analyze_word_error;

mod body;
mod end;
mod linkers;
mod prefix;

/// Convert a `main_tier` CST node into the typed `MainTier` domain model.
///
/// Mirrors the specification in the CHAT manual’s Main Tier chapter by parsing the speaker prefix, body,
/// terminator/postcode tail, and optional media bullet. Diagnostics are reported when optional sections
/// deviate from the expected layout, keeping the eventual `MainTier` instance aligned with the published
/// utterance structure (speaker, colon, content, terminator).
pub fn convert_main_tier_node(
    node: Node,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<MainTier> {
    let child_count = node.child_count();

    let prefix = prefix::parse_prefix(node, source, original_input, errors);
    let mut idx = prefix.idx;

    let body = body::parse_body(node, source, errors, idx);
    idx = body.idx;

    let end = end::parse_end(node, source, original_input, errors, idx);
    idx = end.idx;

    if idx < child_count {
        report_extra_children(node, source, errors, idx);
    }

    // No fabricated speaker fallback: if speaker could not be parsed, skip main-tier construction.
    let speaker = match prefix.speaker.filter(|speaker| !speaker.is_empty()) {
        Some(speaker) => speaker,
        None => return ParseOutcome::rejected(),
    };

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    // Calculate content span: from after the colon to end of tier_body
    // Grammar: main_tier: seq($.star, $.speaker, $.colon, $.tab, $.tier_body)
    // Indices:             0          1           2         3      4
    // content_span includes everything after the colon: tab and tier_body
    let content_span = if let Some(colon_node) = node.child(2) {
        // Content starts after colon and includes tab and tier_body
        let tier_end = node.end_byte();
        Some(Span::new(colon_node.end_byte() as u32, tier_end as u32))
    } else {
        None
    };

    let content = body.content;
    let terminator = end.terminator;
    let bullet = end.bullet;

    let mut main_tier = MainTier::new(speaker, content, terminator)
        .with_span(span)
        .with_speaker_span(prefix.speaker_span)
        .with_linkers(body.linkers)
        .with_postcodes(end.postcodes);

    // Extract a terminal bullet that the greedy contents rule left in content.
    main_tier.content.extract_terminal_bullet();

    if let Some(span) = content_span {
        main_tier = main_tier.with_content_span(span);
    }

    if let Some(lang_code) = body.language_code {
        main_tier = main_tier.with_language_code(lang_code);
    }

    // Bullet: grammar-routed (from utterance_end) takes priority,
    // then promoted bullet (from trailing content InternalBullet).
    if let Some(b) = bullet {
        // Grammar-routed bullet from utterance_end takes priority.
        main_tier = main_tier.with_bullet(b);
    }

    ParseOutcome::parsed(main_tier)
}

/// Emit structural-order diagnostics for trailing unconsumed children.
fn report_extra_children(node: Node, source: &str, errors: &impl ErrorSink, start_idx: usize) {
    let child_count = node.child_count();
    for i in start_idx..child_count {
        if let Some(child) = node.child(i as u32) {
            errors.report(ParseError::new(
                ErrorCode::StructuralOrderError,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                format!(
                    "Unexpected child '{}' at position {} of main_tier",
                    child.kind(),
                    i
                ),
            ));
        }
    }
}

/// Parsed prefix slice (`*`, speaker, `:`, tab) and updated child cursor.
pub(super) struct PrefixData {
    pub speaker: Option<String>,
    pub speaker_span: Span,
    pub idx: usize,
}

/// Parsed main-tier body state plus updated child cursor.
pub(super) struct BodyData {
    pub linkers: Vec<crate::model::Linker>,
    pub language_code: Option<String>,
    pub content: Vec<UtteranceContent>,
    pub idx: usize,
}

/// Parsed terminator/postcode/bullet tail plus updated child cursor.
pub(super) struct EndData {
    pub terminator: Option<Terminator>,
    pub postcodes: Vec<Postcode>,
    pub bullet: Option<crate::model::Bullet>,
    pub idx: usize,
}

/// Emit a child-access failure diagnostic for malformed CST.
fn report_cst_access_error(node: Node, source: &str, errors: &impl ErrorSink, idx: usize) {
    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
        format!("Failed to access child at position {} of main_tier", idx),
    ));
}

/// Report an `ERROR` node using the shared word-level analyzer.
fn report_cst_error_node(child: Node, source: &str, errors: &impl ErrorSink) {
    errors.report(analyze_word_error(child, source));
}

/// Consume one `ERROR` child and report it; returns whether it was handled.
pub(super) fn handle_error_node(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    idx: &mut usize,
) -> bool {
    if node.is_error() {
        report_cst_error_node(node, source, errors);
        *idx += 1;
        true
    } else {
        false
    }
}

/// Report a required-child omission in a user-facing input slice.
pub(super) fn report_missing_child(
    original_input: &str,
    errors: &impl ErrorSink,
    code: ErrorCode,
    message: &str,
) {
    errors.report(ParseError::new(
        code,
        Severity::Error,
        SourceLocation::from_offsets(0, original_input.len()),
        ErrorContext::new(original_input, 0..original_input.len(), ""),
        message,
    ));
}

/// Report an unexpected node kind at a positional slot in `main_tier`.
pub(super) fn report_unexpected_child(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    expected: &str,
    position: usize,
) {
    errors.report(ParseError::new(
        ErrorCode::StructuralOrderError,
        Severity::Error,
        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
        format!(
            "Expected '{}' at position {} of main_tier, found '{}'",
            expected,
            position,
            child.kind()
        ),
    ));
}

/// Wrapper around `report_cst_access_error` used by submodules.
pub(super) fn report_cst_access_failure(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    idx: usize,
) {
    report_cst_access_error(node, source, errors, idx);
}
