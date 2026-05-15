//! Phon project tier hover resolvers: `%modsyl`, `%phosyl`, `%phoaln`.
//!
//! These tiers align to OTHER dependent tiers (`%mod`, `%pho`), not directly
//! to the main tier. Hover shows the word under cursor and its aligned
//! counterpart from the corresponding phonological tier.

use super::helpers::{find_text_item_index_at_offset, position_to_offset};
use crate::alignment::formatters::format_pho_item;
use crate::alignment::types::AlignmentHoverInfo;
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::Position;

/// Build hover info for a `%modsyl` word under the cursor.
///
/// Shows the syllabified target word and the corresponding `%mod` word at the
/// same index position.
pub fn find_modsyl_tier_hover_info(
    utterance: &Utterance,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    let modsyl = utterance.modsyl_tier()?;
    let offset = position_to_offset(document, position);
    let word_idx =
        find_text_item_index_at_offset(modsyl.span, &modsyl.words, offset, document, |word| {
            word.as_str().len()
        })?;
    let word = modsyl.words.get(word_idx)?;

    let mut info = AlignmentHoverInfo::new("Syllabified Target (Model) Phonology", word.as_str());

    // Show aligned %mod word
    if let Some(mod_tier) = utterance.mod_tier()
        && let Some(mod_token) = mod_tier.items.get(word_idx)
    {
        info.aligned_to_mod = Some(format_pho_item(mod_token));
    }

    Some(info)
}

/// Build hover info for a `%phosyl` word under the cursor.
pub fn find_phosyl_tier_hover_info(
    utterance: &Utterance,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    let phosyl = utterance.phosyl_tier()?;
    let offset = position_to_offset(document, position);
    let word_idx =
        find_text_item_index_at_offset(phosyl.span, &phosyl.words, offset, document, |word| {
            word.as_str().len()
        })?;
    let word = phosyl.words.get(word_idx)?;

    let mut info = AlignmentHoverInfo::new("Syllabified Actual (Phone) Production", word.as_str());

    // Show aligned %pho word
    if let Some(pho_tier) = utterance.pho_tier()
        && let Some(pho_token) = pho_tier.items.get(word_idx)
    {
        info.aligned_to_pho = Some(format_pho_item(pho_token));
    }

    Some(info)
}

/// Build hover info for a `%phoaln` word under the cursor.
pub fn find_phoaln_tier_hover_info(
    utterance: &Utterance,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    let phoaln = utterance.phoaln_tier()?;
    let offset = position_to_offset(document, position);
    let word_idx =
        find_text_item_index_at_offset(phoaln.span, &phoaln.words, offset, document, |word| {
            word.to_string().len()
        })?;
    let word = phoaln.words.get(word_idx)?;

    let mut info = AlignmentHoverInfo::new("Phone Alignment", word.to_string());

    // Show aligned %mod and %pho words
    if let Some(mod_tier) = utterance.mod_tier()
        && let Some(mod_token) = mod_tier.items.get(word_idx)
    {
        info.aligned_to_mod = Some(format_pho_item(mod_token));
    }
    if let Some(pho_tier) = utterance.pho_tier()
        && let Some(pho_token) = pho_tier.items.get(word_idx)
    {
        info.aligned_to_pho = Some(format_pho_item(pho_token));
    }

    // Show segment-level details
    let details: Vec<(String, String)> = word
        .pairs
        .iter()
        .map(|pair| ("Segment".to_string(), pair.to_string()))
        .collect();
    if !details.is_empty() {
        info.details = details;
    }

    Some(info)
}
