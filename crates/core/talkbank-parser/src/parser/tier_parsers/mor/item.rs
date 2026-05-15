//! Parsers for `%mor` content items (`mor_content`, post-clitics).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{Mor, MorWord};
use tree_sitter::Node;

use super::word::parse_mor_word;
use crate::node_types as kind;
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Converts a `mor_content` CST node into one `Mor` item.
///
/// **Grammar Rule:**
/// ```text
/// mor_content: $ => seq(
///     field('main', $.mor_word),
///     field('post_clitics', repeat($.mor_post_clitic))
/// )
/// ```
pub fn parse_mor_content(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<Mor> {
    let mut main_word: Option<MorWord> = None;
    let mut post_clitics = Vec::new();

    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                kind::MOR_WORD => {
                    if main_word.is_none()
                        && let ParseOutcome::Parsed(word) = parse_mor_word(child, source, errors)
                    {
                        main_word = Some(word);
                    }
                }
                kind::MOR_POST_CLITIC => {
                    if let ParseOutcome::Parsed(clitic) =
                        parse_mor_post_clitic(child, source, errors)
                        && let Some(clitic) = clitic
                    {
                        post_clitics.push(clitic);
                    }
                }
                _ => {
                    errors.report(unexpected_node_error(child, source, "mor_content"));
                }
            }
        }
        idx += 1;
    }

    let Some(main) = main_word else {
        errors.report(unexpected_node_error(
            node,
            source,
            "mor_content missing main mor_word",
        ));
        return ParseOutcome::rejected();
    };

    ParseOutcome::parsed(Mor::new(main).with_post_clitics(post_clitics))
}

/// Converts one `mor_post_clitic` CST node (`~` + `mor_word`).
fn parse_mor_post_clitic(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<MorWord>> {
    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                kind::TILDE => {}
                kind::MOR_WORD => {
                    if let ParseOutcome::Parsed(word) = parse_mor_word(child, source, errors) {
                        return ParseOutcome::parsed(Some(word));
                    }
                }
                _ => {
                    errors.report(unexpected_node_error(child, source, "mor_post_clitic"));
                }
            }
        }
        idx += 1;
    }

    errors.report(unexpected_node_error(
        node,
        source,
        "mor_post_clitic missing mor_word",
    ));
    ParseOutcome::parsed(None)
}
