//! Per-tier highlight handlers (main, mor, pho, sin, gra).
//!
//! Each `handle_*` function receives the cursor position and alignment data,
//! resolves the item index under the cursor, then collects highlight ranges
//! for the aligned items across all tiers. The main-tier handler fans out to
//! all dependent tiers; dependent-tier handlers resolve back to the main tier
//! and then to sibling tiers.

use tower_lsp::lsp_types::*;

use talkbank_model::Span;
use talkbank_model::alignment::{GraIndex, TierDomain, count_tier_positions_until};
use talkbank_model::model::UtteranceContent;

use crate::alignment::finders::{count_alignable_before, get_alignable_content_by_index};
use crate::backend::utils;

use super::range_finders::{
    find_content_range, find_gra_item_index_at_offset, find_gra_item_range,
    find_mor_item_index_at_offset, find_mor_item_range, find_pho_item_index_at_offset,
    find_pho_item_range, find_sin_item_index_at_offset, find_sin_item_range,
};

/// Reverse-lookup an alignment pair whose `target_index` matches
/// `target_idx`, logging on no-match so operators can spot
/// alignment-cache inconsistencies (KIB-015). Returns the pair's
/// 0-indexed position on hit, `None` on miss.
///
/// Generic over the pair type via the
/// [`talkbank_model::alignment::IndexPair`] trait so main↔{mor, pho,
/// mod, sin} alignments all share the same code path.
fn find_alignment_index_for_target<P: talkbank_model::alignment::IndexPair>(
    pairs: &[P],
    target_idx: usize,
    tier_label: &'static str,
) -> Option<usize> {
    let found = pairs
        .iter()
        .enumerate()
        .find(|(_, pair)| pair.target() == Some(target_idx))
        .map(|(idx, _)| idx);
    if found.is_none() {
        tracing::debug!(
            tier = tier_label,
            target_idx,
            pair_count = pairs.len(),
            "highlights: no alignment pair matches target index (alignment-cache mismatch)",
        );
    }
    found
}

/// Get highlight from main tier - highlights all aligned items
pub(super) fn highlights_from_main_tier(
    utterance: &talkbank_model::model::Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<Vec<DocumentHighlight>> {
    let offset = utils::position_to_offset(document, position) as u32;
    let content_idx = find_content_index_at_offset(&utterance.main.content.content, offset)?;

    if !is_alignable_content(&utterance.main.content.content, content_idx) {
        return None;
    }

    let alignment_idx = count_alignable_before(&utterance.main.content.content, content_idx);

    let mut highlights = Vec::new();

    if let Some(content) = utterance.main.content.content.get(content_idx)
        && let Some(range) = find_content_range(content, document)
    {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        });
    }

    if let Some(alignments) = &utterance.alignments {
        if let Some(mor_align) = &alignments.mor
            && let Some(pair) = mor_align.pairs.get(alignment_idx)
            && let Some(mor_idx) = pair.target_index
            && let Some(mor_tier) = utterance.mor_tier()
            && let Some(range) =
                find_mor_item_range(tree, document, mor_tier.span, mor_idx.as_usize())
        {
            highlights.push(DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::READ),
            });
        }

        if let Some(pho_align) = &alignments.pho
            && let Some(pair) = pho_align.pairs.get(alignment_idx)
            && let Some(pho_idx) = pair.target_index
            && let Some(pho_tier) = utterance.pho()
            && let Some(range) =
                find_pho_item_range(tree, document, pho_tier.span, pho_idx.as_usize())
        {
            highlights.push(DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::READ),
            });
        }

        if let Some(mod_align) = &alignments.mod_
            && let Some(pair) = mod_align.pairs.get(alignment_idx)
            && let Some(mod_idx) = pair.target_index
            && let Some(mod_tier) = utterance.mod_tier()
            && let Some(range) =
                find_pho_item_range(tree, document, mod_tier.span, mod_idx.as_usize())
        {
            highlights.push(DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::READ),
            });
        }

        if let Some(sin_align) = &alignments.sin
            && let Some(pair) = sin_align.pairs.get(alignment_idx)
            && let Some(sin_idx) = pair.target_index
            && let Some(sin_tier) = utterance.sin()
            && let Some(range) =
                find_sin_item_range(tree, document, sin_tier.span, sin_idx.as_usize())
        {
            highlights.push(DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::READ),
            });
        }
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

/// Highlights from mor tier - find corresponding main tier word
pub(super) fn highlights_from_mor_tier(
    utterance: &talkbank_model::model::Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<Vec<DocumentHighlight>> {
    let offset = utils::position_to_offset(document, position) as u32;
    let mor_tier = utterance.mor_tier()?;
    let mor_idx = find_mor_item_index_at_offset(tree, mor_tier.span, offset)?;

    let alignments = utterance.alignments.as_ref()?;
    let mor_align = alignments.mor.as_ref()?;

    let alignment_idx = find_alignment_index_for_target(&mor_align.pairs, mor_idx, "%mor")?;

    let mut highlights = Vec::new();

    if let Some(content) =
        get_alignable_content_by_index(&utterance.main.content.content, alignment_idx)
        && let Some(range) = find_content_range(content, document)
    {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        });
    }

    if let Some(range) = find_mor_item_range(tree, document, mor_tier.span, mor_idx) {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::READ),
        });
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

/// Highlights from pho tier
pub(super) fn highlights_from_pho_tier(
    utterance: &talkbank_model::model::Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<Vec<DocumentHighlight>> {
    let offset = utils::position_to_offset(document, position) as u32;
    let pho_tier = utterance.pho()?;
    let pho_idx = find_pho_item_index_at_offset(tree, pho_tier.span, offset)?;

    let alignments = utterance.alignments.as_ref()?;
    let pho_align = alignments.pho.as_ref()?;

    let alignment_idx = find_alignment_index_for_target(&pho_align.pairs, pho_idx, "%pho")?;

    let mut highlights = Vec::new();

    if let Some(content) =
        get_alignable_content_by_index(&utterance.main.content.content, alignment_idx)
        && let Some(range) = find_content_range(content, document)
    {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        });
    }

    if let Some(range) = find_pho_item_range(tree, document, pho_tier.span, pho_idx) {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::READ),
        });
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

/// Highlights from mod tier
pub(super) fn highlights_from_mod_tier(
    utterance: &talkbank_model::model::Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<Vec<DocumentHighlight>> {
    let offset = utils::position_to_offset(document, position) as u32;
    let mod_tier = utterance.mod_tier()?;
    let mod_idx = find_pho_item_index_at_offset(tree, mod_tier.span, offset)?;

    let alignments = utterance.alignments.as_ref()?;
    let mod_align = alignments.mod_.as_ref()?;

    let alignment_idx = find_alignment_index_for_target(&mod_align.pairs, mod_idx, "%mod")?;

    let mut highlights = Vec::new();

    if let Some(content) =
        get_alignable_content_by_index(&utterance.main.content.content, alignment_idx)
        && let Some(range) = find_content_range(content, document)
    {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        });
    }

    if let Some(range) = find_pho_item_range(tree, document, mod_tier.span, mod_idx) {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::READ),
        });
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

/// Highlights from sin tier
pub(super) fn highlights_from_sin_tier(
    utterance: &talkbank_model::model::Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<Vec<DocumentHighlight>> {
    let offset = utils::position_to_offset(document, position) as u32;
    let sin_tier = utterance.sin()?;
    let sin_idx = find_sin_item_index_at_offset(tree, sin_tier.span, offset)?;

    let alignments = utterance.alignments.as_ref()?;
    let sin_align = alignments.sin.as_ref()?;

    let alignment_idx = find_alignment_index_for_target(&sin_align.pairs, sin_idx, "%sin")?;

    let mut highlights = Vec::new();

    if let Some(content) =
        get_alignable_content_by_index(&utterance.main.content.content, alignment_idx)
        && let Some(range) = find_content_range(content, document)
    {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        });
    }

    if let Some(range) = find_sin_item_range(tree, document, sin_tier.span, sin_idx) {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::READ),
        });
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

/// Highlights from gra tier — trace through `%mor` chunks to the host
/// `%mor` item and then through the main↔mor alignment to the host word.
///
/// Three index spaces are in play (see `crates/chatter-lsp/CLAUDE.md`):
///
/// 1. `gra_pair.mor_chunk_index` is a **chunk** index (0-indexed into the
///    expansion produced by [`MorTier::chunks`]).
/// 2. `mor_align.pairs[_].target_index` is a **`%mor` item** index
///    (0-indexed into `MorTier::items`). Post-clitic chunks share the
///    item index of their host.
/// 3. The `alignment_idx` returned by `mor_align.pairs.iter().enumerate()`
///    is the alignable-content position on the main tier.
///
/// The chunk→item projection goes through [`MorTier::item_index_of_chunk`];
/// without it, the handler would treat the chunk index as an item index and
/// silently highlight the wrong main-tier word whenever a `%gra` relation
/// addresses a post-clitic chunk (e.g. every `it's` / `I'll` / `can't`).
pub(super) fn highlights_from_gra_tier(
    utterance: &talkbank_model::model::Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
) -> Option<Vec<DocumentHighlight>> {
    let offset = utils::position_to_offset(document, position) as u32;
    let gra_tier = utterance.gra_tier()?;
    let gra_idx = find_gra_item_index_at_offset(tree, gra_tier.span, offset)?;

    let alignments = utterance.alignments.as_ref()?;
    let gra_align = alignments.gra.as_ref()?;

    let gra_pair = gra_align
        .pairs
        .iter()
        .find(|p| p.gra_index == Some(GraIndex::new(gra_idx)))?;
    let mor_chunk_idx = gra_pair.mor_chunk_index?;

    let mor_tier = utterance.mor_tier()?;
    let host_item_idx = mor_tier.item_index_of_chunk(mor_chunk_idx.as_usize())?;

    let mor_align = alignments.mor.as_ref()?;
    let alignment_idx =
        find_alignment_index_for_target(&mor_align.pairs, host_item_idx, "%mor (via %gra)")?;

    let mut highlights = Vec::new();

    if let Some(content) =
        get_alignable_content_by_index(&utterance.main.content.content, alignment_idx)
        && let Some(range) = find_content_range(content, document)
    {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::TEXT),
        });
    }

    // %mor highlight spans the host item, not just the post-clitic sub-span.
    // Highlighting the whole `pron|it~aux|be` item is the right affordance:
    // it tells the user "this relation refers to the clitic group on this
    // mor item," which matches how the main-tier highlight works.
    if let Some(range) = find_mor_item_range(tree, document, mor_tier.span, host_item_idx) {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::READ),
        });
    }

    if let Some(range) = find_gra_item_range(tree, document, gra_tier.span, gra_idx) {
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::WRITE),
        });
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

/// Finds content index at offset.
fn find_content_index_at_offset(content: &[UtteranceContent], offset: u32) -> Option<usize> {
    content.iter().enumerate().find_map(|(idx, item)| {
        let span = content_span(item)?;
        if span_contains(span, offset) {
            Some(idx)
        } else {
            None
        }
    })
}

/// Returns whether alignable content.
fn is_alignable_content(content: &[UtteranceContent], index: usize) -> bool {
    let before = count_tier_positions_until(content, index, TierDomain::Mor);
    let after = count_tier_positions_until(content, index + 1, TierDomain::Mor);
    after > before
}

/// Return the source span for a main-tier content item.
fn content_span(content: &UtteranceContent) -> Option<Span> {
    match content {
        UtteranceContent::Word(word) => Some(word.span),
        UtteranceContent::AnnotatedWord(annotated) => Some(annotated.span),
        UtteranceContent::ReplacedWord(replaced) => Some(replaced.span),
        UtteranceContent::Group(group) => Some(group.span),
        UtteranceContent::AnnotatedGroup(annotated) => Some(annotated.span),
        UtteranceContent::PhoGroup(_) => None,
        UtteranceContent::SinGroup(_) => None,
        UtteranceContent::Quotation(_) => None,
        _ => None,
    }
}

/// Return `true` when `outer` fully contains `inner`.
fn span_contains(span: Span, offset: u32) -> bool {
    offset >= span.start && offset <= span.end
}
