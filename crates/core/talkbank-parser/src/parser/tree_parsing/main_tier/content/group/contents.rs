//! Converts `contents` lists inside bracketed groups.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

use crate::error::ErrorSink;
use crate::model::{BracketedItem, UtteranceContent};
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM,
    FALLING_TO_LOW, FALLING_TO_MID, LEVEL_PITCH, NON_COLON_SEPARATOR, OVERLAP_POINT,
    RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR, TAG_MARKER, UNMARKED_ENDING,
    UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use tree_sitter::Node;

use super::nested::parse_nested_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Converts a `contents` CST node into `BracketedItem`s.
///
/// The `contents` rule enumerates the tokens that can live inside bracketed tiers (e.g., `%mor`, `%gra`),
/// including explicit overlap/continuation markers. This parser walks the CST children, decoys whitespace,
/// and delegates to `parse_nested_content` so each nested utterance item ends up in the `BracketedItem`
/// vector reported back to the caller. That way the bracketed tiers keep the same ordering and annotated types
/// described in the manual’s Scoped Symbols chapter.
///
/// **Grammar Rule:**
/// ```text
/// contents: $ => repeat1($.content_item)
/// ```
pub(crate) fn parse_group_contents(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<BracketedItem> {
    let child_count = node.child_count();
    // Pre-allocate: each child is typically one content item
    let mut group_items = Vec::with_capacity(child_count);

    for idx in 0..child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                CONTENT_ITEM => {
                    for content in parse_nested_content(child, source, errors) {
                        if let Some(group_content) = convert_to_group_content(content) {
                            group_items.push(group_content);
                        }
                    }
                }
                OVERLAP_POINT
                | SEPARATOR
                | NON_COLON_SEPARATOR
                | COLON
                | COMMA
                | SEMICOLON
                | TAG_MARKER
                | VOCATIVE_MARKER
                | CA_CONTINUATION_MARKER
                | UNMARKED_ENDING
                | UPTAKE_SYMBOL
                | CA_NO_BREAK
                | CA_TECHNICAL_BREAK
                | RISING_TO_HIGH
                | RISING_TO_MID
                | LEVEL_PITCH
                | FALLING_TO_MID
                | FALLING_TO_LOW => {
                    for content in parse_nested_content(child, source, errors) {
                        if let Some(group_content) = convert_to_group_content(content) {
                            group_items.push(group_content);
                        }
                    }
                }
                // Expected: whitespace between content items (no model representation needed)
                WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(
                        child,
                        source,
                        "contents (expected content_item)",
                    ));
                }
            }
        }
    }

    group_items
}

/// Convert `UtteranceContent` into `BracketedItem` when the content is valid inside a bracketed tier.
///
/// Not all `UtteranceContent` variants can appear in bracketed contexts—bare groups, for example, are disallowed.
/// This helper makes those restrictions explicit and returns `None` when the content should remain in the main tier.
pub(crate) fn convert_to_group_content(content: UtteranceContent) -> Option<BracketedItem> {
    match content {
        UtteranceContent::Word(word) => Some(BracketedItem::Word(word)),
        UtteranceContent::AnnotatedWord(ann) => Some(BracketedItem::AnnotatedWord(ann)),
        UtteranceContent::ReplacedWord(rw) => Some(BracketedItem::ReplacedWord(rw)),
        UtteranceContent::Event(event) => Some(BracketedItem::Event(event)),
        UtteranceContent::AnnotatedEvent(ann) => Some(BracketedItem::AnnotatedEvent(ann)),
        UtteranceContent::Pause(pause) => Some(BracketedItem::Pause(pause)),
        UtteranceContent::AnnotatedAction(ann) => Some(BracketedItem::AnnotatedAction(ann)),
        // Bare groups cannot appear inside bracketed content - they must have annotations
        UtteranceContent::Group(_group) => None,
        UtteranceContent::OverlapPoint(marker) => Some(BracketedItem::OverlapPoint(marker)),
        UtteranceContent::Separator(sep) => Some(BracketedItem::Separator(sep.clone())),
        UtteranceContent::InternalBullet(bullet) => Some(BracketedItem::InternalBullet(bullet)),
        UtteranceContent::Freecode(freecode) => Some(BracketedItem::Freecode(freecode)),
        UtteranceContent::LongFeatureBegin(marker) => Some(BracketedItem::LongFeatureBegin(marker)),
        UtteranceContent::LongFeatureEnd(marker) => Some(BracketedItem::LongFeatureEnd(marker)),
        UtteranceContent::NonvocalBegin(marker) => Some(BracketedItem::NonvocalBegin(marker)),
        UtteranceContent::NonvocalEnd(marker) => Some(BracketedItem::NonvocalEnd(marker)),
        UtteranceContent::NonvocalSimple(marker) => Some(BracketedItem::NonvocalSimple(marker)),
        UtteranceContent::UnderlineBegin(marker) => Some(BracketedItem::UnderlineBegin(marker)),
        UtteranceContent::UnderlineEnd(marker) => Some(BracketedItem::UnderlineEnd(marker)),
        UtteranceContent::OtherSpokenEvent(event) => {
            Some(BracketedItem::OtherSpokenEvent(event.clone()))
        }
        // Groups CAN contain annotated groups (e.g., retraces inside pho groups)
        UtteranceContent::AnnotatedGroup(ann) => Some(BracketedItem::AnnotatedGroup(ann)),
        UtteranceContent::Retrace(retrace) => Some(BracketedItem::Retrace(retrace)),
        UtteranceContent::PhoGroup(pho) => Some(BracketedItem::PhoGroup(pho)),
        UtteranceContent::SinGroup(sin) => Some(BracketedItem::SinGroup(sin)),
        UtteranceContent::Quotation(quot) => Some(BracketedItem::Quotation(quot)),
    }
}
