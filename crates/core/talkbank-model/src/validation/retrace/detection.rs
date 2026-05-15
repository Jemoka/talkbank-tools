//! Retrace marker detection across content structures.
//!
//! Since retraces are first-class `Retrace` variants in both
//! `UtteranceContent` and `BracketedItem`, detection checks for those
//! variants directly rather than inspecting annotation lists.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::model::{BracketedContent, BracketedItem, MainTier, UtteranceContent};

/// Returns whether a main tier contains any retrace content.
///
/// This is the fast-path gate before running more expensive retrace rendering
/// and sequence checks.
pub fn contains_retrace_marker(main_tier: &MainTier) -> bool {
    main_tier
        .content
        .content
        .iter()
        .any(utterance_item_has_retrace)
}

/// Returns whether one utterance-content item is or contains retrace content.
///
/// The check descends into nested bracketed structures as needed.
pub fn utterance_item_has_retrace(item: &UtteranceContent) -> bool {
    match item {
        UtteranceContent::Retrace(_) => true,
        UtteranceContent::AnnotatedGroup(ann) => bracketed_content_has_retrace(&ann.inner.content),
        UtteranceContent::Group(group) => bracketed_content_has_retrace(&group.content),
        UtteranceContent::PhoGroup(pho) => bracketed_content_has_retrace(&pho.content),
        UtteranceContent::SinGroup(sin) => bracketed_content_has_retrace(&sin.content),
        UtteranceContent::Quotation(quot) => bracketed_content_has_retrace(&quot.content),
        _ => false,
    }
}

/// Returns whether bracketed content contains retrace content.
///
/// This helper is shared by both top-level and nested traversal paths.
pub fn bracketed_content_has_retrace(content: &BracketedContent) -> bool {
    content.content.iter().any(bracketed_item_has_retrace)
}

/// Returns whether one bracketed item is or contains retrace content.
///
/// Group-like variants recurse into inner content so nested retraces are not
/// missed during early detection.
pub fn bracketed_item_has_retrace(item: &BracketedItem) -> bool {
    match item {
        BracketedItem::Retrace(_) => true,
        BracketedItem::AnnotatedGroup(ann) => bracketed_content_has_retrace(&ann.inner.content),
        BracketedItem::PhoGroup(pho) => bracketed_content_has_retrace(&pho.content),
        BracketedItem::SinGroup(sin) => bracketed_content_has_retrace(&sin.content),
        BracketedItem::Quotation(quot) => bracketed_content_has_retrace(&quot.content),
        _ => false,
    }
}
