//! `%gra` hover resolver — shows the grammatical relation with dependency context.
//!
//! Resolves the `%gra` relation under the cursor (e.g. `1|2|SUBJ`), then looks
//! up the aligned `%mor` item and main-tier word to provide a rich hover card
//! showing the full dependency triple alongside the word and its morphology.

use super::helpers::{
    find_gra_item_index_at_offset, find_source_index_for_target, format_mor_word_label,
    position_to_offset,
};
use crate::alignment::finders::get_alignable_content_by_index;
use crate::alignment::formatters::{format_content_item, format_mor_item};
use crate::alignment::types::AlignmentHoverInfo;
use talkbank_model::alignment::GraHeadRef;
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::Position;

/// Build hover info for a `%gra` relation under the cursor.
pub fn find_gra_tier_hover_info(
    utterance: &Utterance,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
    _is_translation: bool,
) -> Option<AlignmentHoverInfo> {
    let gra_tier = utterance.gra_tier()?;
    let gra_alignment = utterance
        .alignments
        .as_ref()
        .and_then(|alignments| alignments.gra.as_ref());

    let offset = position_to_offset(document, position);
    let gra_idx = find_gra_item_index_at_offset(tree, gra_tier.span, offset)?;

    // Get the gra relation
    let gra_relation = gra_tier.relations().get(gra_idx)?;

    // Get mor tier to resolve word stems
    let mor_tier = utterance.mor_tier();

    // Find this word's stem from mor tier. The dependent's `index` is
    // 1-indexed over the %mor chunk sequence — `format_mor_word_label`
    // handles the chunk projection, so we pass the raw 1-indexed value.
    let word_stem = format_mor_word_label(mor_tier, gra_relation.index);

    // Find head word's stem from mor tier. `head_ref()` classifies the
    // raw `head` field: 0 is the ROOT sentinel, anything else is a
    // 1-indexed chunk position.
    let head_stem = match gra_relation.head_ref() {
        GraHeadRef::Root => "ROOT".to_string(),
        GraHeadRef::Word(idx) => format_mor_word_label(mor_tier, idx.as_usize()),
    };

    // Find all children (words that depend on this word)
    let children: Vec<String> = gra_tier
        .relations()
        .iter()
        .filter(|r| r.head == gra_relation.index)
        .map(|r| {
            let child_stem = format_mor_word_label(mor_tier, r.index);
            format!("{} ({})", child_stem, r.relation)
        })
        .collect();

    // Build hover info
    let mut details = vec![
        ("Word".to_string(), word_stem),
        ("Relation".to_string(), gra_relation.relation.to_string()),
        ("Head".to_string(), head_stem),
    ];

    // Add children if any
    if !children.is_empty() {
        details.push(("Dependents".to_string(), children.join(", ")));
    }

    let mut info = AlignmentHoverInfo::new(
        "Grammatical Relation",
        format!(
            "{}|{}|{}",
            gra_relation.index, gra_relation.head, gra_relation.relation
        ),
    )
    .with_details(details);

    // Look up the aligned %mor item and main-tier word via `AlignmentSet`.
    //
    // `gra_alignment.pairs` carries `MorChunkIndex` on the source side, so
    // `chunk_idx` here is a position in the `%mor` *chunk* sequence. The
    // main↔%mor alignment is keyed by `%mor` *item* index, and we want the
    // item that *hosts* this chunk (same item for a main chunk and all its
    // post-clitics). Collapse chunks to items via `item_index_of_chunk`
    // before indexing `.items()` or looking up the main alignment. See
    // KIB-017 and the parallel fix sites at `tier_handlers.rs` +
    // `goto_definition.rs`.
    if let Some(gra_alignment) = gra_alignment
        && let Some(chunk_idx) = find_source_index_for_target(&gra_alignment.pairs, gra_idx)
        && let Some(mor_tier) = utterance.mor_tier()
        && let Some(host_item_idx) = mor_tier.item_index_of_chunk(chunk_idx)
        && let Some(mor_item) = mor_tier.items().get(host_item_idx)
    {
        info.aligned_to_mor = Some(format_mor_item(mor_item));

        if let Some(alignments) = utterance.alignments.as_ref()
            && let Some(mor_alignment) = alignments.mor.as_ref()
            && let Some(main_idx) =
                find_source_index_for_target(&mor_alignment.pairs, host_item_idx)
            && let Some(main_content) =
                get_alignable_content_by_index(&utterance.main.content.content, main_idx)
        {
            info.aligned_to_main = Some(format_content_item(main_content));
        }
    }

    Some(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{parse_chat_with_alignments, parse_tree};
    use talkbank_model::model::{ChatFile, Line};

    fn first_utterance(chat_file: &ChatFile) -> &Utterance {
        chat_file
            .lines
            .iter()
            .find_map(|l| {
                if let Line::Utterance(u) = l {
                    Some(u)
                } else {
                    None
                }
            })
            .expect("fixture must contain an utterance")
    }

    /// Hovering a `%gra` relation whose chunk is a post-clitic must resolve
    /// `aligned_to_mor` to the clitic's **host item** (the one that contains
    /// the `~` group), not to the `%mor` item whose 0-indexed position
    /// happens to equal the chunk index. Same bug class as KIB-C001 through
    /// KIB-C003; this site was missed in the 2026-04-16 migration (KIB-017).
    #[test]
    fn gra_hover_on_post_clitic_aligns_to_host_mor_item() {
        // chunks: 0=pron|it, 1=aux|be (post-clitic), 2=n|cookie, 3=terminator.
        // cursor on `%gra` relation `2|0|ROOT` (gra_idx=1 → chunk 1 = aux|be).
        // Correct host item is `pron|it~aux|be`, not `n|cookie`.
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\tit's cookies .\n%mor:\tpron|it~aux|be n|cookie .\n%gra:\t1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT\n@End\n";
        let chat_file = parse_chat_with_alignments(content);
        let tree = parse_tree(content);
        let utterance = first_utterance(&chat_file);

        let cursor_on_2_0_root = Position {
            line: 7,
            character: 18,
        };
        let info = find_gra_tier_hover_info(utterance, &tree, cursor_on_2_0_root, content, false)
            .expect("hover on %gra relation must yield info");
        let aligned_to_mor = info
            .aligned_to_mor
            .as_ref()
            .expect("%gra hover must populate aligned_to_mor when alignment exists");

        // The host item is `pron|it~aux|be`. `format_mor_item` renders the
        // main word's POS tag (`pron`) and appends a "Post-clitic" section
        // for `aux|be`. If the function used the chunk index as an item
        // index it would pick up `n|cookie` instead (noun, no post-clitic).
        assert!(
            aligned_to_mor.contains("pron"),
            "aligned_to_mor should render the host item (pron|it~aux|be); got {aligned_to_mor:?}"
        );
        assert!(
            aligned_to_mor.contains("Post-clitic"),
            "aligned_to_mor should include the post-clitic section of the host item; got {aligned_to_mor:?}"
        );
        assert!(
            !aligned_to_mor.contains("cookie"),
            "aligned_to_mor must not render the next item (n|cookie) when the clitic is the target; got {aligned_to_mor:?}"
        );
    }
}
