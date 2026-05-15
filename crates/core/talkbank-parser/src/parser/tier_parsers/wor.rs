//! Word timing tier (%wor) parser
//!
//! Parses %wor tiers which contain word-level timing annotations.
//!
//! The grammar gives %wor its own `wor_tier_body` rule containing a flat
//! whitespace-separated sequence of `wor_word_item` (standalone words),
//! `inline_bullet` (timing), and tag-marker separators. Words and bullets
//! are siblings — the parser pairs each word with its following bullet.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::node_types::{
    BULLET, COMMA, LANGCODE, NEWLINE, TAG_MARKER, TERMINATOR, VOCATIVE_MARKER, WHITESPACES,
    WOR_TIER_BODY, WOR_WORD_ITEM,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::Bullet;
use talkbank_model::model::dependent_tier::{WorItem, WorTier};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::main_tier::structure::terminator::terminator_from_node_kind;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;
use talkbank_model::ParseOutcome;

/// Converts `%wor` into a `WorTier`.
///
/// The CST is a flat sequence under `wor_tier_body`:
///   wor_word_item whitespaces inline_bullet whitespaces wor_word_item ...
/// Words and bullets are siblings. We pair each word with the next
/// inline_bullet (if any) by walking left-to-right.
pub fn parse_wor_tier(node: Node, source: &str, errors: &impl ErrorSink) -> WorTier {
    let span = talkbank_model::Span::new(node.start_byte() as u32, node.end_byte() as u32);

    // Find wor_tier_body
    let mut cursor = node.walk();
    let wor_body = node
        .children(&mut cursor)
        .find(|c| c.kind() == WOR_TIER_BODY);

    let body = match wor_body {
        Some(b) => b,
        None => return WorTier::new(vec![]).with_span(span),
    };

    let mut items: Vec<WorItem> = Vec::new();
    let mut terminator = None;
    let mut language_code = None;

    let mut body_cursor = body.walk();
    for child in body.children(&mut body_cursor) {
        match child.kind() {
            LANGCODE => {
                if let Some(lc) = extract_langcode(child, source) {
                    language_code = Some(lc);
                }
            }
            WOR_WORD_ITEM => {
                // wor_word_item is just a standalone_word — extract the word
                let word_node = child.child(0);
                if let Some(wn) = word_node
                    && let ParseOutcome::Parsed(w) = convert_word_node(wn, source, errors)
                {
                    items.push(WorItem::Word(Box::new(w)));
                }
            }
            BULLET => {
                // Pair this bullet with the preceding word (if any)
                if let Some(bullet) = parse_inline_bullet(child, source)
                    && let Some(WorItem::Word(w)) = items.last_mut()
                {
                    w.inline_bullet = Some(bullet);
                }
            }
            // Tag-marker separators: comma, tag „, vocative ‡
            COMMA | TAG_MARKER | VOCATIVE_MARKER => {
                let item_span =
                    talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32);
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    items.push(WorItem::Separator {
                        text: text.to_string(),
                        span: item_span,
                    });
                }
            }
            TERMINATOR => {
                // Supertype wrapper — unwrap to get the actual terminator variant
                if let Some(inner) = child.child(0u32) {
                    let term_span = talkbank_model::Span::new(
                        inner.start_byte() as u32,
                        inner.end_byte() as u32,
                    );
                    terminator = terminator_from_node_kind(inner.kind(), term_span);
                }
            }
            kind if terminator_from_node_kind(
                kind,
                talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32),
            )
            .is_some() =>
            {
                // Direct terminator node (not wrapped in supertype)
                let term_span =
                    talkbank_model::Span::new(child.start_byte() as u32, child.end_byte() as u32);
                terminator = terminator_from_node_kind(kind, term_span);
            }
            // Expected: whitespace between content items (no model representation needed)
            WHITESPACES => {}
            // Expected: trailing newline in wor_tier_body grammar rule (no model representation needed)
            NEWLINE => {}
            _ => errors.report(unexpected_node_error(child, source, "wor_tier_body")),
        }
    }

    WorTier::new(items)
        .with_terminator(terminator)
        .with_language_code(language_code)
        .with_span(span)
}

/// Extract language code from an atomic langcode token.
/// Delegates to the shared token parser in the direct parser crate.
fn extract_langcode(node: Node, source: &str) -> Option<talkbank_model::model::LanguageCode> {
    let raw = node.utf8_text(source.as_bytes()).ok()?;
    crate::tokens::parse_langcode_token(raw)
}

/// Parse an inline_bullet node into a Bullet.
///
/// After grammar coarsening, `inline_bullet` is a single token.
fn parse_inline_bullet(node: Node, source: &str) -> Option<Bullet> {
    let (start_ms, end_ms) =
        crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps(node, source)?;
    Some(Bullet::new(start_ms, end_ms))
}
