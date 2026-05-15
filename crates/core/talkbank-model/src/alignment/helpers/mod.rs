//! Shared primitives used by `%mor/%pho/%sin/%wor` alignment passes.
//!
//! These helpers centralize domain policy (what counts as alignable, when to
//! ignore annotations, and how replacement branches expand). Keeping those rules
//! in one place prevents drift between per-tier aligners.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

mod count;
mod domain;
pub mod overlap;
pub mod overlap_groups;
mod rules;
mod walk;

#[cfg(test)]
mod tests;

pub use count::{
    MorAlignableWordCount, MorItemCount, TierPosition, collect_tier_items, count_tier_positions,
    count_tier_positions_until,
};

/// Render any [`WriteChat`](crate::model::WriteChat) value into owned text
/// for alignment diagnostic messages.
///
/// Best-effort: write failures are silently ignored because diagnostic
/// formatting must never panic the alignment path.
pub fn to_chat_display_string<T: crate::model::WriteChat>(item: &T) -> String {
    let mut s = String::new();
    item.write_chat(&mut s).ok();
    s
}
pub use domain::TierDomain;
pub use overlap::{
    OverlapMarkerInfo, OverlapPointVisit, OverlapRegion, OverlapRegionKind, extract_overlap_info,
    walk_overlap_points,
};
pub use overlap_groups::{
    FileOverlapAnalysis, OverlapAnchor, OverlapGroup, PerUtteranceOverlap, analyze_file_overlaps,
};
pub use rules::should_align_replaced_word_in_pho_sin;
pub use rules::{
    annotations_have_alignment_ignore, counts_for_tier, counts_for_tier_in_context,
    is_tag_marker_separator, should_skip_group,
};
pub use walk::{
    ContentItem, ContentItemMut, WordItem, WordItemMut, walk_content, walk_content_mut, walk_words,
    walk_words_mut,
};
