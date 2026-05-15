//! Parsing for `word_with_optional_annotations` content items.
//!
//! Combines the base word token with optional replacement and scoped
//! annotations into one `UtteranceContent` variant.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::ErrorSink;
use crate::model::{
    Annotated, BracketedContent, BracketedItem, ReplacedWord, Retrace, UtteranceContent,
};
use crate::node_types::{BASE_ANNOTATIONS, REPLACEMENT, STANDALONE_WORD, WHITESPACES};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::annotations::{parse_replacement, parse_scoped_annotations};
use super::super::word::convert_word_node;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::expect_child;

/// Converts `word_with_optional_annotations` into `UtteranceContent`.
///
/// **Grammar Rule:**
/// ```text
/// word_with_optional_annotations: $ => seq(
///   $.standalone_word,
///   optional(seq(
///     $.whitespaces,
///     choice($.replacement, $.phonological_replacement)
///   )),
///   optional($.base_annotations)
/// )
/// ```
///
/// **Expected Sequential Order:**
/// 1. `standalone_word`
/// 2. [optional] `whitespaces` followed by `replacement` or `phonological_replacement`
/// 3. [optional] `base_annotations`
pub(crate) fn parse_word_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();
    let mut word = None;
    let mut replacement = ParseOutcome::rejected();
    let mut retrace_kind = None;
    // Pre-allocate: words typically have 0-3 annotations
    let mut annotations = Vec::with_capacity(2);
    let mut idx: u32 = 0;

    // Position 0: standalone_word (required)
    // CRITICAL: Use expect_child to check for MISSING nodes - prevents fake Word objects
    if let ParseOutcome::Parsed(child) = expect_child(
        node,
        idx,
        STANDALONE_WORD,
        source,
        errors,
        "word_with_optional_annotations",
    ) {
        // Reuse our existing word CST conversion for full parsing
        if let ParseOutcome::Parsed(w) = convert_word_node(child, source, errors) {
            word = Some(w);
        }
        idx += 1;
    }

    // Position 1+: optional whitespaces, replacement, base_annotations
    while (idx as usize) < child_count {
        if let Some(child) = node.child(idx) {
            match child.kind() {
                WHITESPACES => {
                    // Whitespace between word parts - expected
                    idx += 1;
                }
                REPLACEMENT => {
                    // Parse replacement [: word1 word2 ...]
                    replacement = parse_replacement(child, source, errors);
                    idx += 1;
                }
                // Note: phonological_replacement was legacy and doesn't exist in current grammar
                BASE_ANNOTATIONS => {
                    // Parse the base_annotations container node
                    let parsed = parse_scoped_annotations(child, source, errors);
                    annotations.extend(parsed.content);
                    if parsed.retrace.is_some() {
                        retrace_kind = parsed.retrace;
                    }
                    idx += 1;
                }
                _ => {
                    // Unexpected child
                    errors.report(unexpected_node_error(
                        child,
                        source,
                        "word_with_optional_annotations",
                    ));
                    idx += 1;
                }
            }
        } else {
            break;
        }
    }

    if let Some(w) = word {
        if let ParseOutcome::Parsed(repl) = replacement {
            // Word with replacement [: ...]
            let replaced = ReplacedWord::new(w, repl).with_scoped_annotations(annotations);
            if let Some(kind) = retrace_kind {
                // Replaced word inside a retrace: word [: replacement] [* error] [//]
                // Wrap the ReplacedWord in a Retrace node so it is excluded from
                // %mor alignment counting.
                let span = replaced.span;
                let bracketed =
                    BracketedContent::new(vec![BracketedItem::ReplacedWord(Box::new(replaced))]);
                let retrace = Retrace::new(bracketed, kind).with_span(span);
                ParseOutcome::parsed(UtteranceContent::Retrace(Box::new(retrace)))
            } else {
                ParseOutcome::parsed(UtteranceContent::ReplacedWord(Box::new(replaced)))
            }
        } else if let Some(kind) = retrace_kind {
            // Single-word retrace: wrap word in BracketedContent
            let span = w.span;
            let bracketed = BracketedContent::new(vec![BracketedItem::Word(Box::new(w))]);
            let retrace = Retrace::new(bracketed, kind)
                .with_annotations(annotations)
                .with_span(span);
            ParseOutcome::parsed(UtteranceContent::Retrace(Box::new(retrace)))
        } else if annotations.is_empty() {
            // Bare word
            ParseOutcome::parsed(UtteranceContent::Word(Box::new(w)))
        } else {
            // Word with non-retrace annotations
            let annotated = Annotated::new(w).with_scoped_annotations(annotations);
            ParseOutcome::parsed(UtteranceContent::AnnotatedWord(Box::new(annotated)))
        }
    } else {
        ParseOutcome::rejected()
    }
}
