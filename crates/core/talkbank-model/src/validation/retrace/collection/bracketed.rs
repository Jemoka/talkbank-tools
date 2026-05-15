//! Leaf-kind collection inside bracketed content trees.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::super::types::{LeafKind, RetraceCheck};
use crate::model::{BracketedContent, BracketedItem};

/// Collect leaf kinds and retrace checkpoints from bracketed content.
///
/// Traversal is depth-first and preserves transcript order so leaf indices stay
/// compatible with retrace rendering and validation passes.
pub fn collect_bracketed_content(
    content: &BracketedContent,
    leaf_kinds: &mut Vec<LeafKind>,
    retrace_checks: &mut Vec<RetraceCheck>,
    retrace_index: &mut usize,
) {
    for item in content.content.iter() {
        collect_bracketed_item(item, leaf_kinds, retrace_checks, retrace_index);
    }
}

/// Collect leaf kinds and retrace checkpoints from one bracketed item.
///
/// Behavior mirrors utterance-level collection so nested content and top-level
/// content share identical retrace semantics.
pub fn collect_bracketed_item(
    item: &BracketedItem,
    leaf_kinds: &mut Vec<LeafKind>,
    retrace_checks: &mut Vec<RetraceCheck>,
    retrace_index: &mut usize,
) {
    match item {
        BracketedItem::Word(_) => leaf_kinds.push(LeafKind::RealContent),
        BracketedItem::AnnotatedWord(_) => {
            leaf_kinds.push(LeafKind::RealContent);
        }
        BracketedItem::ReplacedWord(_) => {
            leaf_kinds.push(LeafKind::RealContent);
        }
        BracketedItem::Event(_) => leaf_kinds.push(LeafKind::RealContent),
        BracketedItem::AnnotatedEvent(_) => {
            leaf_kinds.push(LeafKind::RealContent);
        }
        BracketedItem::Pause(_) => leaf_kinds.push(LeafKind::RealContent),
        BracketedItem::OtherSpokenEvent(_) => leaf_kinds.push(LeafKind::RealContent),
        BracketedItem::AnnotatedGroup(ann) => {
            collect_bracketed_content(
                &ann.inner.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
        }
        BracketedItem::PhoGroup(pho) => {
            collect_bracketed_content(&pho.content, leaf_kinds, retrace_checks, retrace_index);
        }
        BracketedItem::SinGroup(sin) => {
            collect_bracketed_content(&sin.content, leaf_kinds, retrace_checks, retrace_index);
        }
        BracketedItem::Quotation(quot) => {
            collect_bracketed_content(&quot.content, leaf_kinds, retrace_checks, retrace_index);
        }
        BracketedItem::Retrace(retrace) => {
            collect_bracketed_content(&retrace.content, leaf_kinds, retrace_checks, retrace_index);
            // Record retrace check at the content variant level
            retrace_checks.push(RetraceCheck {
                retrace_index: *retrace_index,
                after_leaf_index: leaf_kinds.len(),
            });
            *retrace_index += 1;
        }
        BracketedItem::Separator(_)
        | BracketedItem::OverlapPoint(_)
        | BracketedItem::InternalBullet(_)
        | BracketedItem::Freecode(_)
        | BracketedItem::LongFeatureBegin(_)
        | BracketedItem::LongFeatureEnd(_)
        | BracketedItem::UnderlineBegin(_)
        | BracketedItem::UnderlineEnd(_)
        | BracketedItem::NonvocalBegin(_)
        | BracketedItem::NonvocalEnd(_)
        | BracketedItem::NonvocalSimple(_)
        | BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_) => {
            leaf_kinds.push(LeafKind::NonRealContent);
        }
    }
}
