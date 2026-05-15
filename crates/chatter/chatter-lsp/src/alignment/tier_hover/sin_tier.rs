//! `%sin` hover resolver — shows gesture/sign details with aligned main-tier word.
//!
//! Resolves the `%sin` item under the cursor, extracts structured gesture
//! details (type, lexeme, discriminator point), and looks up the aligned
//! main-tier word to provide context for the annotation.

use super::helpers::{
    find_sin_item_index_at_offset, find_source_index_for_target, position_to_offset,
};
use crate::alignment::finders::get_alignable_content_by_index;
use crate::alignment::formatters::{format_content_item, format_sin_item_details};
use crate::alignment::types::AlignmentHoverInfo;
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::Position;

/// Build hover info for a `%sin` item under the cursor.
pub fn find_sin_tier_hover_info(
    utterance: &Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    // Get sin tier and alignment
    let sin_tier = utterance.sin()?;
    let sin_alignment = utterance.alignments.as_ref()?.sin.as_ref()?;

    let offset = position_to_offset(document, position);
    let sin_idx = find_sin_item_index_at_offset(tree, sin_tier.span, offset)?;

    // Get the sin item
    let sin_item = sin_tier.items.get(sin_idx)?;

    // Format gesture information
    let (element_content, details) = format_sin_item_details(sin_item);

    // Build hover info
    let mut info =
        AlignmentHoverInfo::new("Gesture/Sign Annotation", element_content).with_details(details);

    // Look up alignment to main tier
    if let Some(main_idx) = find_source_index_for_target(&sin_alignment.pairs, sin_idx)
        && let Some(main_content) =
            get_alignable_content_by_index(&utterance.main.content.content, main_idx)
    {
        info.aligned_to_main = Some(format_content_item(main_content));
    }

    Some(info)
}
