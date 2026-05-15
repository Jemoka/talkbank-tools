//! `%pho` hover resolver — shows phonological transcription with aligned main-tier word.
//!
//! Resolves the `%pho` item under the cursor and looks up the corresponding
//! main-tier word via `AlignmentSet.pho` to display both the phonological form
//! and the orthographic word it annotates.

use super::helpers::{
    find_pho_item_index_at_offset, find_source_index_for_target, position_to_offset,
};
use crate::alignment::finders::get_alignable_content_by_index;
use crate::alignment::formatters::{format_content_item, format_pho_item};
use crate::alignment::types::AlignmentHoverInfo;
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::Position;

/// Build hover info for a `%pho` item under the cursor.
pub fn find_pho_tier_hover_info(
    utterance: &Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    // Get pho tier and alignment
    let pho_tier = utterance.pho()?;
    let pho_alignment = utterance.alignments.as_ref()?.pho.as_ref()?;

    let offset = position_to_offset(document, position);
    let pho_idx = find_pho_item_index_at_offset(tree, pho_tier.span, offset)?;

    // Get the pho token
    let pho_token = pho_tier.items.get(pho_idx)?;

    // Build hover info
    let mut info =
        AlignmentHoverInfo::new("Phonological Transcription", format_pho_item(pho_token));

    // Look up alignment to main tier
    if let Some(main_idx) = find_source_index_for_target(&pho_alignment.pairs, pho_idx)
        && let Some(main_content) =
            get_alignable_content_by_index(&utterance.main.content.content, main_idx)
    {
        info.aligned_to_main = Some(format_content_item(main_content));
    }

    Some(info)
}
