//! Leaf-kind collection over top-level utterance content.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::super::types::{LeafKind, RetraceCheck};
use super::bracketed;
use crate::model::UtteranceContent;

/// Collect leaf kinds and retrace checkpoints from one utterance-content item.
///
/// The function recursively descends into grouped/bracketed payloads while
/// keeping retrace indices monotonic across the whole utterance traversal.
pub fn collect_utterance_content(
    item: &UtteranceContent,
    leaf_kinds: &mut Vec<LeafKind>,
    retrace_checks: &mut Vec<RetraceCheck>,
    retrace_index: &mut usize,
) {
    match item {
        UtteranceContent::Word(_) => leaf_kinds.push(LeafKind::RealContent),
        UtteranceContent::AnnotatedWord(_) => {
            leaf_kinds.push(LeafKind::RealContent);
        }
        UtteranceContent::ReplacedWord(_) => {
            leaf_kinds.push(LeafKind::RealContent);
        }
        UtteranceContent::Event(_) => leaf_kinds.push(LeafKind::RealContent),
        UtteranceContent::AnnotatedEvent(_) => {
            leaf_kinds.push(LeafKind::RealContent);
        }
        UtteranceContent::Pause(_) => leaf_kinds.push(LeafKind::RealContent),
        UtteranceContent::OtherSpokenEvent(_) => leaf_kinds.push(LeafKind::RealContent),
        UtteranceContent::Group(group) => {
            bracketed::collect_bracketed_content(
                &group.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
        }
        UtteranceContent::AnnotatedGroup(ann) => {
            bracketed::collect_bracketed_content(
                &ann.inner.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
        }
        UtteranceContent::PhoGroup(pho) => {
            bracketed::collect_bracketed_content(
                &pho.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
        }
        UtteranceContent::SinGroup(sin) => {
            bracketed::collect_bracketed_content(
                &sin.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
        }
        UtteranceContent::Quotation(quot) => {
            bracketed::collect_bracketed_content(
                &quot.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
        }
        UtteranceContent::Retrace(retrace) => {
            bracketed::collect_bracketed_content(
                &retrace.content,
                leaf_kinds,
                retrace_checks,
                retrace_index,
            );
            // Record retrace check at the content variant level
            retrace_checks.push(RetraceCheck {
                retrace_index: *retrace_index,
                after_leaf_index: leaf_kinds.len(),
            });
            *retrace_index += 1;
        }
        UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::AnnotatedAction(_) => {
            leaf_kinds.push(LeafKind::NonRealContent);
        }
    }
}
