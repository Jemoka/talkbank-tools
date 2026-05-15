//! Retrace leaf-kind collection orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

mod bracketed;
mod utterance;

use super::types::{LeafKind, RetraceCheck};
use crate::model::MainTier;
use utterance::collect_utterance_content;

/// Collect leaf classifications and retrace checkpoints from one main tier.
///
/// The returned `LeafKind` stream represents serialized content order; retrace
/// checks store the leaf index each retrace marker follows.
pub fn collect_retrace_checks(main_tier: &MainTier) -> (Vec<LeafKind>, Vec<RetraceCheck>) {
    let mut leaf_kinds = Vec::new();
    let mut retrace_checks = Vec::new();
    let mut retrace_index = 0usize;

    for item in main_tier.content.content.iter() {
        collect_utterance_content(
            item,
            &mut leaf_kinds,
            &mut retrace_checks,
            &mut retrace_index,
        );
    }

    if main_tier.content.terminator.is_some() {
        leaf_kinds.push(LeafKind::Terminator);
    }

    (leaf_kinds, retrace_checks)
}
