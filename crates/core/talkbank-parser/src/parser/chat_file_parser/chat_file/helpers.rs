//! Shared low-level routines used by CHAT file parsing.
//!
//! This layer handles line iteration, selective top-level error recovery, and
//! conversion into `Line` values before participant synthesis and normalization.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::{ChatDate, Header, Line, WarningText};
use crate::node_types::*;
use crate::parser::TreeSitterParser;
use crate::parser::chat_file_parser::header_parser::{handle_pre_begin_header, parse_header_node};
use crate::parser::chat_file_parser::utterance_parser::{
    classify_percent_error_text, parse_utterance_node,
};
use crate::parser::tree_parsing::parser_helpers::{
    analyze_error_node, analyze_line_error, is_header, is_pre_begin_header,
};
use talkbank_model::ParseOutcome;
use tracing::{debug, info, trace, warn};

/// Recover specific top-level `ERROR` nodes that still encode a valid header shape.
fn recover_top_level_error_node(
    error_node: tree_sitter::Node,
    input: &str,
    lines: &mut Vec<Line>,
) -> bool {
    let Ok(text) = error_node.utf8_text(input.as_bytes()) else {
        return false;
    };

    let bytes = text.as_bytes();
    if bytes.starts_with(b"@Date:")
        && bytes[6..]
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
        let span = Span::new(error_node.start_byte() as u32, error_node.end_byte() as u32);
        let date_value = text[6..].trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r'));
        lines.push(Line::header_with_span(
            Header::Date {
                date: ChatDate::new(date_value),
            },
            span,
        ));
        return true;
    }

    if recover_unknown_header_line(error_node, text, lines) {
        return true;
    }

    false
}

/// Convert an unknown `@Header:` line embedded in an `ERROR` node into `Header::Unknown`.
fn recover_unknown_header_line(
    error_node: tree_sitter::Node,
    text: &str,
    lines: &mut Vec<Line>,
) -> bool {
    let first_line = match text.lines().next() {
        Some(line) => line.trim_end(),
        None => return false,
    };
    if !first_line.starts_with('@') {
        return false;
    }

    let colon_index = match first_line.find(':') {
        Some(idx) => idx,
        None => return false,
    };
    if colon_index <= 1 {
        return false;
    }

    let label = &first_line[1..colon_index];
    if is_known_header_label(label) {
        return false;
    }

    let span = Span::new(error_node.start_byte() as u32, error_node.end_byte() as u32);
    lines.push(Line::header_with_span(
        Header::Unknown {
            text: WarningText::new(first_line.to_string()),
            parse_reason: Some(format!(
                "Recovered unknown header '{}' from parse error node",
                label
            )),
            suggested_fix: Some(
                "Use a standard CHAT header, or keep this as legacy metadata".to_string(),
            ),
        },
        span,
    ));
    true
}

/// Return whether `label` is a known CHAT header key.
fn is_known_header_label(label: &str) -> bool {
    matches!(
        label.to_ascii_lowercase().as_str(),
        "utf8"
            | "begin"
            | "end"
            | "new episode"
            | "languages"
            | "comment"
            | "participants"
            | "id"
            | "pid"
            | "date"
            | "media"
            | "number"
            | "recording quality"
            | "transcription"
            | "situation"
            | "types"
            | "tape location"
            | "time duration"
            | "time start"
            | "birth of"
            | "birthplace of"
            | "l1 of"
            | "font"
            | "window"
            | "color words"
            | "bck"
            | "bg"
            | "eg"
            | "g"
            | "t"
            | "location"
            | "room layout"
            | "transcriber"
            | "videos"
            | "options"
            | "warning"
            | "activities"
            | "blank"
            | "page"
    )
}

/// Report malformed/orphaned top-level dependent tiers and taint the prior utterance if present.
fn report_top_level_dependent_tier_error(
    error_node: tree_sitter::Node,
    input: &str,
    lines: &mut [Line],
    errors: &impl ErrorSink,
) -> bool {
    let Ok(text) = error_node.utf8_text(input.as_bytes()) else {
        return false;
    };

    if !text.starts_with('%') {
        return false;
    }

    let first_line = text.lines().next().unwrap_or(text);
    let mut has_preceding_utterance = false;
    if let Some(utterance) = lines.iter_mut().rev().find_map(|line| match line {
        Line::Utterance(utt) => Some(utt),
        _ => None,
    }) {
        has_preceding_utterance = true;
        match classify_percent_error_text(first_line) {
            Some(tier) => utterance.mark_parse_taint(tier),
            None => utterance.mark_all_dependent_alignment_taint(),
        }
    }

    let (code, message, suggestion) = if !has_preceding_utterance {
        (
            ErrorCode::OrphanedDependentTier,
            format!(
                "Dependent tier appears before any main tier: {}",
                first_line.trim_end()
            ),
            "Move this dependent tier directly below its parent main tier",
        )
    } else if !first_line.contains(":\t") {
        (
            ErrorCode::MalformedTierHeader,
            format!("Malformed dependent tier header: {}", first_line.trim_end()),
            "Use dependent tier syntax %tier:\\tcontent",
        )
    } else if first_line.contains("|||") {
        (
            ErrorCode::InvalidDependentTier,
            format!("Invalid dependent tier content: {}", first_line.trim_end()),
            "Provide valid tier content for the declared dependent tier type",
        )
    } else if first_line.starts_with("%mor:")
        || first_line.starts_with("%gra:")
        || first_line.starts_with("%pho:")
        || first_line.starts_with("%sin:")
    {
        (
            ErrorCode::TierValidationError,
            format!(
                "Tier validation error: could not fully parse dependent tier '{}'",
                first_line.trim_end()
            ),
            "Fix tier-internal format — check tokenization, pipe delimiters, and required fields",
        )
    } else {
        (
            ErrorCode::InvalidDependentTier,
            format!(
                "Could not fully parse dependent tier: {}",
                first_line.trim_end()
            ),
            "Check dependent tier syntax (%tier:\\tcontent) and tier-specific format",
        )
    };

    errors.report(
        ParseError::new(
            code,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(input, error_node.start_byte()..error_node.end_byte(), text),
            message,
        )
        .with_suggestion(suggestion),
    );

    true
}

/// Parse all lines from `input` and stream diagnostics to `errors`.
pub(super) fn parse_lines(
    parser: &TreeSitterParser,
    input: &str,
    errors: &impl ErrorSink,
) -> Vec<Line> {
    parse_lines_with_old_tree(parser, input, None, errors).0
}

/// Parse lines, optionally reusing `old_tree` for incremental updates.
/// Returns `(lines, new_tree)`.
pub(super) fn parse_lines_with_old_tree(
    parser: &TreeSitterParser,
    input: &str,
    old_tree: Option<&tree_sitter::Tree>,
    errors: &impl ErrorSink,
) -> (Vec<Line>, Option<tree_sitter::Tree>) {
    debug!("Parsing CHAT file ({} bytes)", input.len());

    let tree = match parser.parser.borrow_mut().parse(input, old_tree) {
        Some(t) => t,
        None => {
            warn!("Tree-sitter parse failed for CHAT file");
            errors.report(ParseError::new(
                ErrorCode::ParseFailed,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Tree-sitter parse failed for chat file",
            ));
            return (Vec::new(), None);
        }
    };
    let tree_to_return = tree.clone();

    trace!("Tree-sitter parse completed");
    let ts_root = tree.root_node();

    // With multi-root grammar, root is `source_file` containing `full_document`.
    // Navigate to `full_document` if present, otherwise use root directly.
    let root_node = if ts_root.kind() == "source_file" {
        ts_root
            .child(0)
            .filter(|c| c.kind() == "full_document")
            .unwrap_or(ts_root)
    } else {
        ts_root
    };

    // Check if the root node itself has errors AND is empty (e.g., empty file)
    if root_node.has_error() && root_node.child_count() == 0 {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len().max(1)),
                ErrorContext::new(input, 0..input.len().max(1), input),
                "Unparsable content: file is empty or contains no recognizable CHAT structure",
            )
            .with_suggestion("CHAT files must contain at minimum @UTF8, @Begin, and @End headers"),
        );
        return (Vec::new(), None);
    }

    // Track whether root is ERROR — we'll need this after the loop to report
    // an error if no valid lines were recovered.
    let root_is_error = root_node.is_error();

    let mut lines = Vec::with_capacity(root_node.child_count());

    let mut cursor = root_node.walk();
    for child in root_node.children(&mut cursor) {
        if child.is_error() {
            if report_top_level_dependent_tier_error(child, input, &mut lines, errors) {
                continue;
            }
            if recover_top_level_error_node(child, input, &mut lines) {
                continue;
            }
            analyze_error_node(child, input, errors);
            continue;
        }
        if child.is_missing() {
            continue;
        }

        match child.kind() {
            UTF8_HEADER => {
                let span = Span::new(child.start_byte() as u32, child.end_byte() as u32);
                lines.push(Line::header_with_span(Header::Utf8, span));
            }
            BEGIN_HEADER => {
                let span = Span::new(child.start_byte() as u32, child.end_byte() as u32);
                lines.push(Line::header_with_span(Header::Begin, span));
            }
            END_HEADER => {
                let span = Span::new(child.start_byte() as u32, child.end_byte() as u32);
                lines.push(Line::header_with_span(Header::End, span));
            }

            // Handle both PRE_BEGIN_HEADER wrapper and concrete pre-begin header types
            kind if kind == PRE_BEGIN_HEADER || is_pre_begin_header(kind) => {
                if kind == PRE_BEGIN_HEADER {
                    let mut pre_cursor = child.walk();
                    for pre_child in child.children(&mut pre_cursor) {
                        let span =
                            Span::new(pre_child.start_byte() as u32, pre_child.end_byte() as u32);
                        handle_pre_begin_header(pre_child, span, input, errors, &mut lines);
                    }
                } else {
                    let span = Span::new(child.start_byte() as u32, child.end_byte() as u32);
                    handle_pre_begin_header(child, span, input, errors, &mut lines);
                }
            }

            NEWLINE => {}

            LINE => {
                let mut line_cursor = child.walk();
                for line_child in child.children(&mut line_cursor) {
                    if line_child.is_error() {
                        analyze_line_error(line_child, child, input, errors);
                        continue;
                    }
                    if line_child.is_missing() {
                        continue;
                    }

                    let line_kind = line_child.kind();
                    // Check for header (wrapper or concrete type due to supertypes)
                    if line_kind == HEADER || is_header(line_kind) {
                        if let ParseOutcome::Parsed(header) =
                            parse_header_node(line_child, input, errors)
                        {
                            let span = Span::new(
                                line_child.start_byte() as u32,
                                line_child.end_byte() as u32,
                            );
                            lines.push(Line::header_with_span(header, span));
                        }
                    } else if line_kind == UTTERANCE {
                        if let ParseOutcome::Parsed(utt) =
                            parse_utterance_node(line_child, input, errors)
                        {
                            lines.push(Line::utterance(utt));
                        }
                    } else if line_kind == UNSUPPORTED_LINE {
                        // Catch-all for junk lines — log a warning and skip
                        let text = line_child
                            .utf8_text(input.as_bytes())
                            .unwrap_or("<invalid UTF-8>");
                        errors.report(ParseError::new(
                            ErrorCode::UnexpectedLineType,
                            Severity::Error,
                            SourceLocation::from_offsets(
                                line_child.start_byte(),
                                line_child.end_byte(),
                            ),
                            ErrorContext::new(
                                input,
                                line_child.start_byte()..line_child.end_byte(),
                                UNSUPPORTED_LINE,
                            ),
                            format!("Unsupported line skipped: {}", text.trim()),
                        ));
                    } else {
                        errors.report(ParseError::new(
                            ErrorCode::UnexpectedLineType,
                            Severity::Error,
                            SourceLocation::from_offsets(
                                line_child.start_byte(),
                                line_child.end_byte(),
                            ),
                            ErrorContext::new(
                                input,
                                line_child.start_byte()..line_child.end_byte(),
                                line_kind,
                            ),
                            format!("Unknown node type '{}' in line", line_kind),
                        ));
                    }
                }
            }
            other => {
                if !matches!(other, NEWLINE | WHITESPACES) {
                    trace!("Unhandled top-level node type: {}", other);
                }
            }
        }
    }

    // When the root IS an ERROR node and the loop couldn't recover any valid
    // lines, the file is completely unparsable.  Report this so the strict caller
    // returns Err.  When the root is ERROR but children ARE valid structures
    // (e.g., missing @End), the loop recovers lines and the validation layer
    // can catch the missing header.
    if root_is_error && lines.is_empty() {
        errors.report(
            ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len().max(1)),
                ErrorContext::new(input, 0..input.len().max(1), input),
                "Unparsable content: file structure is not valid CHAT and no lines could be recovered",
            )
            .with_suggestion("CHAT files must contain @UTF8, @Begin, @Participants, @Languages, and @End headers"),
        );
    }

    info!("Parsed {} lines", lines.len());

    (lines, Some(tree_to_return))
}
