//! Body extraction for `main_tier` conversion.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::{UtteranceContent, Word};
use crate::node_types::{
    CONTENTS, LANGCODE, LINKERS, OVERLAP_POINT, SPACE, TIER_BODY, UTTERANCE_END, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::content::analyze_word_error;
use super::super::contents::parse_main_tier_contents;
use super::BodyData;
use super::linkers::parse_linkers;

/// Parse the main-tier body for utterance linkers, optional language switches, and content items.
///
/// The CHAT main tier supports utterance linkers (`++`, `+<`, `+"`, etc.), optional language-code
/// tokens, and the primary `contents` block. This function walks either the new `tier_body` wrapper
/// or the legacy CST structure, records those elements, and delegates to `parse_main_tier_contents`
/// so each `UtteranceContent` matches the grammar described in the Main Tier chapter.
pub(super) fn parse_body(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
    mut idx: usize,
) -> BodyData {
    let child_count = node.child_count();
    let mut linkers: Vec<crate::model::Linker> = Vec::new();
    let mut language_code: Option<String> = None;
    let mut content = Vec::new();

    // Check if we have a tier_body wrapper node (new unified grammar)
    if let Some(child) = node.child(idx as u32)
        && child.kind() == TIER_BODY
    {
        // Parse tier_body's children instead of main_tier's children
        idx += 1; // Move past tier_body for return value
        return parse_tier_body_children(child, source, errors, idx);
    }

    // Legacy parsing for old grammar structure (or error recovery)
    while idx < child_count {
        let child = match node.child(idx as u32) {
            Some(c) => c,
            None => {
                errors.report(ParseError::new(
                    ErrorCode::StructuralOrderError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                    format!("Failed to access child at position {} of main_tier", idx),
                ));
                idx += 1;
                continue;
            }
        };

        if handle_error_node_with_recovery(child, source, errors, &mut idx, &mut content) {
            continue;
        }

        match child.kind() {
            WHITESPACES | SPACE => {
                idx += 1;
            }
            // Error recovery path: tier_body can follow an ERROR node.
            // Parse it as the main body instead of reporting structural order noise.
            TIER_BODY => {
                return parse_tier_body_children(child, source, errors, idx + 1);
            }
            LINKERS => {
                linkers = parse_linkers(child, source, errors);
                idx += 1;
            }
            LANGCODE => {
                // Delegate to shared langcode token parser
                if let Ok(raw) = child.utf8_text(source.as_bytes())
                    && let Some(lc) = crate::tokens::parse_langcode_token(raw)
                {
                    language_code = Some(lc.to_string());
                }
                if language_code.is_none() {
                    errors.report(ParseError::new(
                        ErrorCode::StructuralOrderError,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                        "Malformed language code".to_string(),
                    ));
                }
                idx += 1;
            }
            OVERLAP_POINT => {
                // Overlap points can appear before contents (CA markers)
                // Parse as utterance content and add to content vector
                use crate::parser::tree_parsing::main_tier::content::parse_overlap_point;
                if let ParseOutcome::Parsed(overlap) = parse_overlap_point(child, source, errors) {
                    content.push(overlap);
                }
                idx += 1;
            }
            CONTENTS => {
                // Add any parsed contents from the contents node
                let mut contents_items = parse_main_tier_contents(child, source, errors);
                content.append(&mut contents_items);
                idx += 1;
                continue;
            }
            UTTERANCE_END => {
                break;
            }
            _ => {
                errors.report(ParseError::new(
                    ErrorCode::StructuralOrderError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!("Unexpected child '{}' in main_tier", child.kind()),
                ));
                idx += 1;
            }
        }
    }

    BodyData {
        linkers,
        language_code,
        content,
        idx,
    }
}

/// Parse children of `tier_body` (new unified grammar introduced for `main_tier`).
///
/// When the CST includes a `tier_body` node, it consolidates the same elements documented in the
/// Main Tier section (linkers, language codes, contents). This helper retains the same behavior by
/// sharing the parser for each element, ensuring `BodyData` stays consistent regardless of grammar changes.
fn parse_tier_body_children(
    tier_body_node: Node,
    source: &str,
    errors: &impl ErrorSink,
    parent_idx: usize,
) -> BodyData {
    let child_count = tier_body_node.child_count();
    let mut linkers: Vec<crate::model::Linker> = Vec::new();
    let mut language_code: Option<String> = None;
    let mut content = Vec::new();
    let mut idx = 0;

    while idx < child_count {
        let child = match tier_body_node.child(idx as u32) {
            Some(c) => c,
            None => {
                idx += 1;
                continue;
            }
        };

        if handle_error_node_with_recovery(child, source, errors, &mut idx, &mut content) {
            continue;
        }

        match child.kind() {
            WHITESPACES | SPACE => {
                idx += 1;
            }
            LINKERS => {
                linkers = parse_linkers(child, source, errors);
                idx += 1;
            }
            LANGCODE => {
                // Delegate to shared langcode token parser
                if let Ok(raw) = child.utf8_text(source.as_bytes())
                    && let Some(lc) = crate::tokens::parse_langcode_token(raw)
                {
                    language_code = Some(lc.to_string());
                }
                if language_code.is_none() {
                    errors.report(ParseError::new(
                        ErrorCode::StructuralOrderError,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                        "Malformed language code".to_string(),
                    ));
                }
                idx += 1;
            }
            CONTENTS => {
                let mut contents_items = parse_main_tier_contents(child, source, errors);
                content.append(&mut contents_items);
                idx += 1;
                continue;
            }
            UTTERANCE_END => {
                break;
            }
            _ => {
                errors.report(ParseError::new(
                    ErrorCode::StructuralOrderError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!("Unexpected child '{}' in tier_body", child.kind()),
                ));
                idx += 1;
            }
        }
    }

    BodyData {
        linkers,
        language_code,
        content,
        idx: parent_idx, // Return parent's idx (position after tier_body)
    }
}

/// Handle `ERROR` nodes with suffix-attachment and lexical fallback recovery.
fn handle_error_node_with_recovery(
    child: Node,
    source: &str,
    errors: &impl ErrorSink,
    idx: &mut usize,
    content: &mut Vec<UtteranceContent>,
) -> bool {
    if !child.is_error() {
        return false;
    }

    if attach_error_suffix_to_previous_word(child, source, content) {
        *idx += 1;
        return true;
    }

    if recover_error_as_word(child, source, content) {
        *idx += 1;
        return true;
    }

    // Preserve parser diagnostics without fabricating model placeholders.
    errors.report(analyze_word_error(child, source));
    *idx += 1;
    true
}

/// Attach compact marker fragments (for example `@xyz`) to the previous word token.
fn attach_error_suffix_to_previous_word(
    error_node: Node,
    source: &str,
    content: &mut [UtteranceContent],
) -> bool {
    let Ok(error_text) = error_node.utf8_text(source.as_bytes()) else {
        return false;
    };

    if error_text.is_empty() || error_text.bytes().any(|b| b.is_ascii_whitespace()) {
        return false;
    }

    let Some(last) = content.last_mut() else {
        return false;
    };

    let should_attach = match last {
        UtteranceContent::Word(word) => {
            error_text.starts_with('@')
                || (word.raw_text().contains('@')
                    && error_text.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || matches!(b, b':' | b'+' | b'&' | b'-' | b'_')
                    }))
        }
        UtteranceContent::AnnotatedWord(annotated) => {
            error_text.starts_with('@')
                || (annotated.inner.raw_text().contains('@')
                    && error_text.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || matches!(b, b':' | b'+' | b'&' | b'-' | b'_')
                    }))
        }
        UtteranceContent::ReplacedWord(replaced) => {
            error_text.starts_with('@')
                || (replaced.word.raw_text().contains('@')
                    && error_text.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || matches!(b, b':' | b'+' | b'&' | b'-' | b'_')
                    }))
        }
        _ => false,
    };

    if should_attach {
        match last {
            UtteranceContent::Word(word) => {
                let new_raw = format!("{}{}", word.raw_text(), error_text);
                word.set_raw_text(new_raw);
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                let new_raw = format!("{}{}", annotated.inner.raw_text(), error_text);
                annotated.inner.set_raw_text(new_raw);
            }
            UtteranceContent::ReplacedWord(replaced) => {
                let new_raw = format!("{}{}", replaced.word.raw_text(), error_text);
                replaced.word.set_raw_text(new_raw);
            }
            _ => return false,
        }
        true
    } else {
        false
    }
}

/// Recover standalone lexical `ERROR` fragments as conservative `Word` nodes.
fn recover_error_as_word(
    error_node: Node,
    source: &str,
    content: &mut Vec<UtteranceContent>,
) -> bool {
    let Ok(error_text) = error_node.utf8_text(source.as_bytes()) else {
        return false;
    };

    if error_text.is_empty() || error_text.bytes().any(|b| b.is_ascii_whitespace()) {
        return false;
    }

    // Conservative lexical recovery for dropped tokens after media bullets.
    let looks_like_word = error_text
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'\'' | b'-' | b'_' | b'`'));
    if !looks_like_word {
        return false;
    }

    let span = crate::error::Span::from_usize(error_node.start_byte(), error_node.end_byte());
    let word = Word::new_unchecked(error_text, error_text).with_span(span);
    content.push(UtteranceContent::Word(Box::new(word)));
    true
}
