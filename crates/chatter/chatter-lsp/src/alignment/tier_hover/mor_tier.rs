//! `%mor` hover resolver — shows morphology with aligned main-tier word and `%gra`.
//!
//! Resolves the `%mor` item under the cursor, looks up the aligned main-tier
//! word (via `AlignmentSet.mor`), and optionally includes the `%gra` relation
//! to give a complete word-level picture.

use super::helpers::{
    find_mor_item_index_at_offset, find_source_index_for_target, find_target_index_for_source,
    format_gra_alignment_text, position_to_offset,
};
use crate::alignment::finders::get_alignable_content_by_index;
use crate::alignment::formatters::{format_content_item, format_mor_item};
use crate::alignment::types::AlignmentHoverInfo;
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::Position;

/// Build hover info for a `%mor` item under the cursor.
pub fn find_mor_tier_hover_info(
    utterance: &Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
    _is_translation: bool,
) -> Option<AlignmentHoverInfo> {
    let mor_tier = utterance.mor_tier()?;
    let mor_alignment = utterance
        .alignments
        .as_ref()
        .and_then(|alignments| alignments.mor.as_ref());

    let offset = position_to_offset(document, position);
    let mor_idx = find_mor_item_index_at_offset(tree, mor_tier.span, offset)?;

    // Get the mor item
    let mor_item = mor_tier.items().get(mor_idx)?;

    // Build hover info
    let mut info = AlignmentHoverInfo::new("Morphology Element", format_mor_item(mor_item));

    // Backward lookup to main tier via AlignmentSet.
    if let Some(mor_alignment) = mor_alignment
        && let Some(main_idx) = find_source_index_for_target(&mor_alignment.pairs, mor_idx)
        && let Some(main_content) =
            get_alignable_content_by_index(&utterance.main.content.content, main_idx)
    {
        info.aligned_to_main = Some(format_content_item(main_content));
    }

    // Forward lookup to %gra via AlignmentSet.
    if let Some(alignments) = &utterance.alignments
        && let Some(gra_alignment) = alignments.gra.as_ref()
        && let Some(gra_idx) = find_target_index_for_source(&gra_alignment.pairs, mor_idx)
        && let Some(gra_tier) = utterance.gra_tier()
        && let Some(gra_relation) = gra_tier.relations().get(gra_idx)
    {
        info.aligned_to_gra = Some(format_gra_alignment_text(mor_tier, gra_relation));
    }

    Some(info)
}
