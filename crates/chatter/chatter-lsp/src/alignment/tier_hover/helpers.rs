//! Shared lookup helpers for tier hover resolvers.
//!
//! Provides CST-based `find_*_item_index_at_offset` helpers, alignment-pair
//! lookup utilities, text-tier offset resolution, and shared `%mor`/`%gra`
//! formatting helpers so each tier-specific resolver can stay focused on its
//! own composition rules.

use crate::backend::utils;
use talkbank_model::Span;
use talkbank_model::alignment::{GraHeadRef, IndexPair};
use talkbank_model::model::{GrammaticalRelation, MorTier};
use talkbank_parser::node_types::{GRA_RELATION, MOR_CONTENT, PHO_GROUP, SIN_GROUP};
use tower_lsp::lsp_types::Position;
use tree_sitter::{Node, Tree};

/// Format a `%mor` chunk label for a 1-indexed semantic word position from `%gra`.
///
/// `%gra` relation indices address the `%mor` **chunk** sequence (each item's
/// main word, then its post-clitics, then the terminator), not the smaller
/// `items` list. The actual chunk walk lives in `talkbank-model` as
/// [`MorTier::chunk_at`] — this helper is a thin presentation adapter that
/// converts the 1-indexed semantic position into the 0-indexed chunk index
/// the model expects and falls back to a `"word N"` placeholder when the
/// index is out of range (e.g. points at the terminator, which has no lemma,
/// or past the end of the tier).
pub fn format_mor_word_label(mor_tier: Option<&MorTier>, word_index: usize) -> String {
    mor_tier
        .zip(word_index.checked_sub(1))
        .and_then(|(mor, chunk_idx)| mor.chunk_at(chunk_idx))
        .and_then(|chunk| chunk.lemma().map(str::to_owned))
        .unwrap_or_else(|| format!("word {word_index}"))
}

/// Format the `%gra` summary shown alongside aligned `%mor`/main-tier hover info.
pub fn format_gra_alignment_text(mor_tier: &MorTier, relation: &GrammaticalRelation) -> String {
    let (head_word, head_role) = match relation.head_ref() {
        GraHeadRef::Root => ("ROOT".to_string(), "root of sentence"),
        GraHeadRef::Word(idx) => (
            format!(
                "{} (word {idx})",
                format_mor_word_label(Some(mor_tier), idx.as_usize()),
            ),
            "dependent",
        ),
    };

    format!("{} → {head_word} ({head_role})", relation.relation)
}

/// Convert an LSP position into a document byte offset.
pub fn position_to_offset(document: &str, position: Position) -> u32 {
    utils::position_to_offset(document, position) as u32
}

/// Find the source-side index aligned to `target_index`.
pub fn find_source_index_for_target<P: IndexPair>(
    pairs: &[P],
    target_index: usize,
) -> Option<usize> {
    pairs
        .iter()
        .find(|pair| pair.target() == Some(target_index))
        .and_then(|pair| pair.source())
}

/// Find the target-side index aligned to `source_index`.
pub fn find_target_index_for_source<P: IndexPair>(
    pairs: &[P],
    source_index: usize,
) -> Option<usize> {
    pairs
        .iter()
        .find(|pair| pair.source() == Some(source_index))
        .and_then(|pair| pair.target())
}

/// Find `%mor` item index at `offset` within `tier_span`.
pub fn find_mor_item_index_at_offset(tree: &Tree, tier_span: Span, offset: u32) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, MOR_CONTENT, offset)
}

/// Find `%pho` item index at `offset` within `tier_span`.
pub fn find_pho_item_index_at_offset(tree: &Tree, tier_span: Span, offset: u32) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, PHO_GROUP, offset)
}

/// Find `%sin` item index at `offset` within `tier_span`.
pub fn find_sin_item_index_at_offset(tree: &Tree, tier_span: Span, offset: u32) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, SIN_GROUP, offset)
}

/// Find `%gra` relation index at `offset` within `tier_span`.
pub fn find_gra_item_index_at_offset(tree: &Tree, tier_span: Span, offset: u32) -> Option<usize> {
    find_node_index_at_offset(tree, tier_span, GRA_RELATION, offset)
}

/// Find which rendered text item contains `offset` inside a text-only tier span.
pub fn find_text_item_index_at_offset<T>(
    span: Span,
    items: &[T],
    offset: u32,
    document: &str,
    item_len: impl Fn(&T) -> usize,
) -> Option<usize> {
    let tier_text = document.get(span.start as usize..span.end as usize)?;
    let content_start = tier_text.find(":\t").map(|p| p + 2)?;
    let content_offset = span.start + u32::try_from(content_start).ok()?;
    let content = &tier_text[content_start..];

    let mut pos = 0usize;
    for (idx, item) in items.iter().enumerate() {
        while pos < content.len() && content.as_bytes().get(pos) == Some(&b' ') {
            pos += 1;
        }

        let rendered_len = item_len(item);
        let item_start = content_offset + u32::try_from(pos).ok()?;
        let item_end = item_start + u32::try_from(rendered_len).ok()?;
        if offset >= item_start && offset <= item_end {
            return Some(idx);
        }

        pos += rendered_len;
    }

    None
}

/// Generic node-index resolver for tier-local items of one tree-sitter kind.
fn find_node_index_at_offset(
    tree: &Tree,
    tier_span: Span,
    kind: &str,
    offset: u32,
) -> Option<usize> {
    let root = tree.root_node();
    let mut stack = vec![root];
    let mut current_idx = 0usize;

    while let Some(node) = stack.pop() {
        if !node_overlaps_span(node, tier_span) {
            continue;
        }

        if !node.is_missing() && node.kind() == kind && node_inside_span(node, tier_span) {
            let start = node.start_byte() as u32;
            let end = node.end_byte() as u32;
            if offset >= start && offset <= end {
                return Some(current_idx);
            }
            current_idx += 1;
        }

        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }

    None
}

/// Return `true` when a tree-sitter node intersects the given span.
fn node_overlaps_span(node: Node, span: Span) -> bool {
    let start = node.start_byte() as u32;
    let end = node.end_byte() as u32;
    end >= span.start && start <= span.end
}

/// Return `true` when a tree-sitter node is fully inside the given span.
fn node_inside_span(node: Node, span: Span) -> bool {
    let start = node.start_byte() as u32;
    let end = node.end_byte() as u32;
    start >= span.start && end <= span.end
}

#[cfg(test)]
mod tests {
    use super::{
        find_source_index_for_target, find_target_index_for_source, find_text_item_index_at_offset,
        format_mor_word_label,
    };
    use talkbank_model::Span;
    use talkbank_model::alignment::{AlignmentPair, GraAlignmentPair};
    use talkbank_model::model::{Mor, MorTier, MorWord, PosCategory};

    #[test]
    fn source_lookup_matches_target_index_in_pair_not_row_position() {
        let pairs = vec![
            GraAlignmentPair::from_raw(None, Some(0)),
            GraAlignmentPair::from_raw(Some(0), Some(1)),
        ];

        assert_eq!(find_source_index_for_target(&pairs, 1), Some(0));
        assert_eq!(find_source_index_for_target(&pairs, 0), None);
    }

    #[test]
    fn target_lookup_matches_source_index_in_pair_not_row_position() {
        let pairs = vec![
            AlignmentPair::new(None, Some(0)),
            AlignmentPair::new(Some(0), Some(1)),
        ];

        assert_eq!(find_target_index_for_source(&pairs, 0), Some(1));
    }

    #[test]
    fn text_item_lookup_tracks_offsets_inside_tier_content() {
        let document = "%modsyl:\tab cde";
        let span = Span::from_usize(0, document.len());
        let items = ["ab", "cde"];

        assert_eq!(
            find_text_item_index_at_offset(span, &items, 10, document, |item| item.len()),
            Some(0)
        );
        assert_eq!(
            find_text_item_index_at_offset(span, &items, 13, document, |item| item.len()),
            Some(1)
        );
    }

    /// A `%gra` relation on a post-clitic must resolve to the clitic's lemma,
    /// not to the next `%mor` item's lemma.
    ///
    /// Fixture models `*CHI: it's cookies .` / `%mor: pron|it~aux|be noun|cookie .`
    /// which expands to four `%gra` chunks: `it` (item 0), `be` (post-clitic
    /// of item 0), `cookie` (item 1), terminator. Semantic word indices used
    /// by `%gra` relations are 1-indexed over that chunk sequence, so word 2
    /// is the `be` clitic — not `cookie`.
    ///
    /// Before the fix, `format_mor_word_label` indexed `mor.items` directly
    /// with `word_index - 1`, so word 2 returned `"cookie"` and word 3 fell
    /// off the end into a `"word 3"` fallback. Root cause: semantic word
    /// indices address mor *chunks*, not mor *items*.
    #[test]
    fn gra_word_label_with_post_clitic_resolves_to_clitic_lemma() {
        let its = Mor::new(MorWord::new(PosCategory::new("pron"), "it"))
            .with_post_clitic(MorWord::new(PosCategory::new("aux"), "be"));
        let cookie = Mor::new(MorWord::new(PosCategory::new("noun"), "cookie"));
        let mor = MorTier::new_mor(
            vec![its, cookie],
            talkbank_model::Terminator::Period {
                span: talkbank_model::Span::DUMMY,
            },
        );

        // Semantic word 1 → chunk 0 → main of item 0 → "it". Already correct today.
        assert_eq!(format_mor_word_label(Some(&mor), 1), "it");

        // Semantic word 2 → chunk 1 → post-clitic of item 0 → "be".
        // BUG: currently returns "cookie" because the function indexes mor.items[1].
        assert_eq!(format_mor_word_label(Some(&mor), 2), "be");

        // Semantic word 3 → chunk 2 → main of item 1 → "cookie".
        // BUG: currently returns "word 3" because mor.items[2] is None.
        assert_eq!(format_mor_word_label(Some(&mor), 3), "cookie");
    }
}
