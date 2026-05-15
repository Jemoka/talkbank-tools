//! LSP hover support for cross-tier alignment in CHAT utterances.
//!
//! Alignment semantics are defined in `talkbank-model::alignment`; this module
//! is strictly a presentation/query layer on top of those model results.
//!
//! # Architecture
//!
//! 1. Map cursor position → UtteranceContent item (using parsed data model)
//! 2. Determine alignment index (using count_tier_positions logic)
//! 3. Prefer embedded alignment state on model nodes (word/chunk), with legacy metadata fallback
//! 4. Format hover info from parsed tier data
//!
//! # Guardrails
//!
//! - Do not re-tokenize raw text in LSP code.
//! - Do not duplicate model alignment logic here.
//! - Preserve model handling of fragments, compounds, clitics, and replacements.
//! - Treat alignment as index-based metadata, not naive word-position matching.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod finders;
pub mod formatters;
#[cfg(test)]
mod tests;
mod tier_hover;
mod types;

pub use formatters::format_alignment_info;
pub use types::AlignmentHoverInfo;

use talkbank_model::Span;
use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::{ChatFile, Utterance};
use tower_lsp::lsp_types::Position;
use tree_sitter::Tree;

use crate::backend::utils;
use tier_hover::{
    find_gra_tier_hover_info, find_main_tier_hover_info, find_modsyl_tier_hover_info,
    find_mor_tier_hover_info, find_pho_tier_hover_info, find_phoaln_tier_hover_info,
    find_phosyl_tier_hover_info, find_sin_tier_hover_info,
};

/// Compute alignment hover information for an LSP cursor position.
///
/// This is the main alignment-hover entry point used by the backend hover feature.
///
/// # Arguments
///
/// * `chat_file` - The parsed CHAT file
/// * `position` - LSP cursor position (line, character)
/// * `document` - The raw document text (for line offset calculation)
///
/// # Returns
///
/// `Some(AlignmentHoverInfo)` if the cursor is on a supported alignable element;
/// otherwise `None`.
pub fn find_alignment_hover_info(
    chat_file: &ChatFile,
    tree: &Tree,
    position: Position,
    document: &str,
) -> Option<AlignmentHoverInfo> {
    // Find which utterance contains this position
    let utterance = utils::find_utterance_at_position(chat_file, position, document)?;
    let offset = utils::position_to_offset(document, position) as u32;

    if span_contains(utterance.main.span, offset) {
        return find_main_tier_hover_info(utterance, tree, position, document);
    }

    let tier = find_dependent_tier_at_offset(utterance, offset)?;
    match tier {
        DependentTier::Mor(_) => {
            find_mor_tier_hover_info(utterance, tree, position, document, false)
        }
        DependentTier::Gra(_) => {
            find_gra_tier_hover_info(utterance, tree, position, document, false)
        }
        DependentTier::Pho(_) => find_pho_tier_hover_info(utterance, tree, position, document),
        DependentTier::Sin(_) => find_sin_tier_hover_info(utterance, tree, position, document),
        DependentTier::Modsyl(_) => find_modsyl_tier_hover_info(utterance, position, document),
        DependentTier::Phosyl(_) => find_phosyl_tier_hover_info(utterance, position, document),
        DependentTier::Phoaln(_) => find_phoaln_tier_hover_info(utterance, position, document),
        _ => None,
    }
}

/// Return the dependent tier containing `offset`, if any.
///
/// Dependent tiers follow the CHAT manual’s tier ordering (`%mor`, `%gra`, `%pho`, `%sin`, etc.).
/// This helper looks up the tier whose span contains the cursor so alignment hover information can
/// surface the same tier-specific guidance later in the pipeline.
fn find_dependent_tier_at_offset(utterance: &Utterance, offset: u32) -> Option<&DependentTier> {
    utterance
        .dependent_tiers
        .iter()
        .find(|tier| dependent_tier_span(tier).is_some_and(|span| span_contains(span, offset)))
}

/// Return the source span for a dependent tier variant.
///
/// Each dependent tier keeps track of its byte span so hover information can show which tier the cursor
/// sits in (helpful for explaining `%mor` vs `%gra` alignment errors as described in the manual).
fn dependent_tier_span(tier: &DependentTier) -> Option<Span> {
    Some(tier.span())
}

/// Return `true` when `offset` lies within `span`.
///
/// Basic utility used to ensure the hover logic follows the CHAT span semantics for tiers and utterances.
fn span_contains(span: Span, offset: u32) -> bool {
    offset >= span.start && offset <= span.end
}
