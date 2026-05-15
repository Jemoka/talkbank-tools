//! Helpers for parsing grouped `%pho` content.
//!
//! These routines decode either flat `pho_words` or bracketed grouped phonology
//! (`‹ ... ›`) into `PhoItem` values while preserving fallback text when needed.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

use crate::node_types as kind;
use talkbank_model::ErrorSink;
use talkbank_model::model::{PhoItem, PhoWord};
use tree_sitter::Node;

use super::cst::{build_group_from_words, fallback_group_as_text};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;

/// Extracts `PhoItem`s from one `pho_group` CST node.
///
/// **Grammar Rule:**
/// ```text
/// pho_group: choice(pho_words, seq('‹', pho_grouped_content, '›'))
/// pho_words: seq(pho_word, repeat(seq('+', pho_word)))
/// ```
///
/// Returns a vector of PhoItems (usually just one)
pub(super) fn extract_pho_group_items(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<PhoItem> {
    // Check first child to determine structure
    if let Some(first_child) = node.child(0u32) {
        match first_child.kind() {
            kind::PHO_WORDS => {
                // Extract text from pho_words (handles pho_word + '+' + pho_word structure)
                let text = extract_utf8_text(first_child, source, errors, "pho_words", "");
                if !text.is_empty() {
                    vec![PhoItem::Word(PhoWord::new(text))]
                } else {
                    vec![]
                }
            }
            kind::PHO_BEGIN_GROUP => {
                // Grouped content: ‹ pho_grouped_content ›
                // Parse the grouped content into a PhoGroup
                if let Some(pho_grouped_content) = node.child(1u32) {
                    if pho_grouped_content.kind() == kind::PHO_GROUPED_CONTENT {
                        let words =
                            extract_pho_grouped_content_words(pho_grouped_content, source, errors);
                        build_group_from_words(words)
                    } else {
                        // Fallback: preserve entire group as text
                        fallback_group_as_text(node, source, errors)
                    }
                } else {
                    vec![]
                }
            }
            _ => fallback_group_as_text(node, source, errors),
        }
    } else {
        vec![]
    }
}

/// Extracts grouped phonology words from `pho_grouped_content`.
///
/// **Grammar Rule:**
/// ```text
/// pho_grouped_content: seq(pho_words, repeat(seq(whitespaces, pho_words)))
/// ```
pub(super) fn extract_pho_grouped_content_words<'a>(
    node: Node<'a>,
    source: &'a str,
    errors: &impl ErrorSink,
) -> Vec<&'a str> {
    let mut words = Vec::new();
    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                kind::PHO_WORDS => {
                    let text = extract_utf8_text(child, source, errors, "pho_words", "");
                    if !text.is_empty() {
                        words.push(text);
                    }
                }
                kind::WHITESPACES => {
                    // Skip whitespace separators
                }
                _ => {
                    errors.report(unexpected_node_error(child, source, "pho_grouped_content"));
                }
            }
        }
        idx += 1;
    }

    words
}
