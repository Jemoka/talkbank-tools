//! Goto-definition logic for CHAT files.
//!
//! Supports two cross-tier jumps:
//! - Speaker code (`*CHI:`) → `@Participants` header definition
//! - `%mor` / `%gra` item → aligned main-tier word (via `AlignmentSet`)

use talkbank_model::Span;
use talkbank_model::model::{ChatFile, UtteranceContent};
use talkbank_parser::node_types::{
    GRA_CONTENTS, GRA_DEPENDENT_TIER, GRA_RELATION, MOR_CONTENT, MOR_CONTENTS, MOR_DEPENDENT_TIER,
    PARTICIPANT, PARTICIPANTS_HEADER, SPEAKER,
};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

use crate::alignment::finders::get_alignable_content_by_index;
use crate::backend::utils;

/// Handles goto definition.
pub(crate) fn goto_definition(
    chat_file: &ChatFile,
    uri: &Url,
    doc: &str,
    tree: &Tree,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let offset = utils::position_to_offset(doc, position);
    let root = tree.root_node();
    let node = root.descendant_for_byte_range(offset, offset);

    // Check if we're on a speaker code (e.g., *CHI:)
    if let Some(node) = node
        && let Some(speaker_node) = find_ancestor_kind(node, SPEAKER)
        && let Ok(speaker_text) = speaker_node.utf8_text(doc.as_bytes())
        && let Some(location) = find_participant_definition_in_tree(tree, uri, doc, speaker_text)
    {
        return Some(GotoDefinitionResponse::Scalar(location));
    }

    // Check if we're on a dependent tier line
    if let Some(location) = find_aligned_definition(chat_file, uri, doc, tree, position) {
        return Some(GotoDefinitionResponse::Scalar(location));
    }

    None
}

// ---------------------------------------------------------------------------
// Tree-sitter node utilities (used across request handlers)
// ---------------------------------------------------------------------------

/// Finds child by kind.
pub(crate) fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

/// Finds ancestor kind.
pub(crate) fn find_ancestor_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut current = Some(node);
    while let Some(node) = current {
        if node.kind() == kind {
            return Some(node);
        }
        current = node.parent();
    }
    None
}

/// Walks nodes depth-first, returning the first `Some` value from the callback.
pub(crate) fn walk_nodes<'a, T>(
    root: Node<'a>,
    mut f: impl FnMut(Node<'a>) -> Option<T>,
) -> Option<T> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(result) = f(node) {
            return Some(result);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    None
}

/// Finds the index of the child with the given kind that contains the offset.
fn find_index_in_children(node: Node, kind: &str, offset: usize) -> Option<usize> {
    let mut cursor = node.walk();
    let mut index = 0;
    for child in node.children(&mut cursor) {
        if child.kind() != kind {
            continue;
        }
        if offset >= child.start_byte() && offset < child.end_byte() {
            return Some(index);
        }
        index += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Speaker definition lookup
// ---------------------------------------------------------------------------

/// Find the @Participants definition for a speaker.
fn find_participant_definition_in_tree(
    tree: &Tree,
    uri: &Url,
    doc: &str,
    speaker: &str,
) -> Option<Location> {
    let root = tree.root_node();
    walk_nodes(root, |node| {
        if node.kind() != PARTICIPANTS_HEADER {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != PARTICIPANT {
                continue;
            }

            if let Some(code_node) = find_child_by_kind(child, SPEAKER)
                && let Ok(code_text) = code_node.utf8_text(doc.as_bytes())
                && code_text == speaker
            {
                return Some(Location {
                    uri: uri.clone(),
                    range: Range {
                        start: utils::offset_to_position(doc, code_node.start_byte() as u32),
                        end: utils::offset_to_position(doc, code_node.end_byte() as u32),
                    },
                });
            }
        }

        None
    })
}

// ---------------------------------------------------------------------------
// Alignment-based definition lookup
// ---------------------------------------------------------------------------

/// Find the aligned main tier word for a dependent tier position.
fn find_aligned_definition(
    chat_file: &ChatFile,
    uri: &Url,
    doc: &str,
    tree: &Tree,
    position: Position,
) -> Option<Location> {
    let offset = utils::position_to_offset(doc, position);
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;

    if find_ancestor_kind(node, MOR_DEPENDENT_TIER).is_some() {
        return find_mor_aligned_definition(chat_file, uri, doc, tree, position);
    }

    if find_ancestor_kind(node, GRA_DEPENDENT_TIER).is_some() {
        return find_gra_aligned_definition(chat_file, uri, doc, tree, position);
    }

    None
}

fn find_mor_item_index_at_offset(tree: &Tree, offset: usize) -> Option<usize> {
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;
    let contents = find_ancestor_kind(node, MOR_CONTENTS)?;
    find_index_in_children(contents, MOR_CONTENT, offset)
}

fn find_gra_relation_index_at_offset(tree: &Tree, offset: usize) -> Option<usize> {
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;
    let contents = find_ancestor_kind(node, GRA_CONTENTS)?;
    find_index_in_children(contents, GRA_RELATION, offset)
}

/// Find aligned main tier word from %mor position.
fn find_mor_aligned_definition(
    chat_file: &ChatFile,
    uri: &Url,
    doc: &str,
    tree: &Tree,
    position: Position,
) -> Option<Location> {
    let (_utterance_idx, utterance) =
        find_utterance_at_position_with_index(chat_file, doc, position)?;

    let mor_tier = utterance.mor_tier()?;
    let mor_alignment = utterance.alignments.as_ref()?.mor.as_ref()?;

    if mor_tier.items().is_empty() {
        return None;
    }

    let offset = utils::position_to_offset(doc, position);
    let mor_idx = find_mor_item_index_at_offset(tree, offset)?;

    // Bounds-check explicitly rather than clamping. Clamping would
    // silently convert a broken CST lookup into a jump to the last
    // %mor item — see KIB-015. An out-of-range cursor means the
    // cursor wasn't over a %mor item, so declining the jump is
    // the right answer.
    if mor_idx >= mor_tier.items().len() {
        tracing::debug!(
            mor_idx,
            tier_len = mor_tier.items().len(),
            "goto-definition: %mor item index out of range for tier; declining jump",
        );
        return None;
    }

    let alignment_pair = mor_alignment.pairs.get(mor_idx)?;
    let main_idx = alignment_pair.source_index?.as_usize();
    find_main_tier_word_location(uri, doc, utterance, main_idx)
}

/// Find aligned word from %gra position.
fn find_gra_aligned_definition(
    chat_file: &ChatFile,
    uri: &Url,
    doc: &str,
    tree: &Tree,
    position: Position,
) -> Option<Location> {
    let (_utterance_idx, utterance) =
        find_utterance_at_position_with_index(chat_file, doc, position)?;

    let gra_tier = utterance.gra_tier()?;
    let gra_alignment = utterance.alignments.as_ref()?.gra.as_ref()?;
    let mor_alignment = utterance.alignments.as_ref()?.mor.as_ref()?;

    if gra_tier.relations().is_empty() {
        return None;
    }

    let offset = utils::position_to_offset(doc, position);
    let gra_idx = find_gra_relation_index_at_offset(tree, offset)?;
    let gra_idx = gra_idx.min(gra_tier.relations().len().saturating_sub(1));

    let gra_pair = gra_alignment.pairs.get(gra_idx)?;
    let mor_chunk_idx = gra_pair.mor_chunk_index?;

    // %gra relations address the %mor *chunk* sequence, but the
    // main↔%mor alignment is keyed by %mor *item* positions. Collapse
    // post-clitic chunks to their host item before indexing — see
    // `crates/chatter-lsp/CLAUDE.md` for the three index spaces and the
    // 2026-04-16 bug that motivated this projection.
    let mor_tier = utterance.mor_tier()?;
    let host_item_idx = mor_tier.item_index_of_chunk(mor_chunk_idx.as_usize())?;

    let mor_pair = mor_alignment.pairs.get(host_item_idx)?;
    let main_idx = mor_pair.source_index?.as_usize();
    find_main_tier_word_location(uri, doc, utterance, main_idx)
}

// ---------------------------------------------------------------------------
// Utterance / content helpers
// ---------------------------------------------------------------------------

/// Find utterance at a given position.
fn find_utterance_at_position_with_index<'a>(
    chat_file: &'a ChatFile,
    doc: &str,
    position: Position,
) -> Option<(usize, &'a talkbank_model::model::Utterance)> {
    let offset = utils::position_to_offset(doc, position) as u32;
    for (idx, utterance) in chat_file.utterances().enumerate() {
        if utterance_contains_offset(utterance, offset) {
            return Some((idx, utterance));
        }
    }
    None
}

/// Find the location of a word in the main tier.
fn find_main_tier_word_location(
    uri: &Url,
    doc: &str,
    utterance: &talkbank_model::model::Utterance,
    word_idx: usize,
) -> Option<Location> {
    let content = get_alignable_content_by_index(&utterance.main.content.content, word_idx)?;
    let span = content_span(content)?;

    Some(Location {
        uri: uri.clone(),
        range: Range {
            start: utils::offset_to_position(doc, span.start),
            end: utils::offset_to_position(doc, span.end),
        },
    })
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

/// Return `true` when an absolute byte offset falls inside an utterance span.
fn utterance_contains_offset(utterance: &talkbank_model::model::Utterance, offset: u32) -> bool {
    let span_contains = |span: Span| offset >= span.start && offset <= span.end;
    span_contains(utterance.main.span)
        || utterance
            .dependent_tiers
            .iter()
            .any(|tier| span_contains(tier.span()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_parser::TreeSitterParser;

    #[test]
    fn test_find_mor_item_index_at_offset() -> std::result::Result<(), String> {
        let input =
            "@UTF8\n@Begin\n*CHI:\tI want cookie .\n%mor:\tpro:sub|I v|want n|cookie .\n@End\n";
        let parser =
            TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
        let tree = parser
            .parse_tree_incremental(input, None)
            .map_err(|_| "Failed to parse CST".to_string())?;

        let root = tree.root_node();
        let mut mor_contents_node = None;
        walk_nodes(root, |node| {
            if node.kind() == MOR_CONTENTS {
                mor_contents_node = Some(node);
                return Some(());
            }
            None::<()>
        });

        let mor_contents =
            mor_contents_node.ok_or_else(|| "Could not find MOR_CONTENTS node".to_string())?;

        let mut mor_nodes = Vec::new();
        let mut cursor = mor_contents.walk();
        for child in mor_contents.children(&mut cursor) {
            if child.kind() == MOR_CONTENT {
                mor_nodes.push(child);
            }
        }

        for (idx, node) in mor_nodes.iter().enumerate() {
            let offset = node.start_byte() + 1;
            assert_eq!(
                find_mor_item_index_at_offset(&tree, offset),
                Some(idx),
                "Failed for node {} at offset {}",
                idx,
                offset
            );
        }
        Ok(())
    }

    #[test]
    fn test_find_gra_relation_index_at_offset() -> std::result::Result<(), String> {
        let input =
            "@UTF8\n@Begin\n*CHI:\tI want cookie .\n%gra:\t1|2|SUBJ 2|0|ROOT 3|2|OBJ .\n@End\n";
        let parser =
            TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
        let tree = parser
            .parse_tree_incremental(input, None)
            .map_err(|_| "Failed to parse CST".to_string())?;

        let root = tree.root_node();
        let mut gra_contents_node = None;
        walk_nodes(root, |node| {
            if node.kind() == GRA_CONTENTS {
                gra_contents_node = Some(node);
                return Some(());
            }
            None::<()>
        });

        let gra_contents =
            gra_contents_node.ok_or_else(|| "Could not find GRA_CONTENTS node".to_string())?;

        let mut gra_nodes = Vec::new();
        let mut cursor = gra_contents.walk();
        for child in gra_contents.children(&mut cursor) {
            if child.kind() == GRA_RELATION {
                gra_nodes.push(child);
            }
        }

        for (idx, node) in gra_nodes.iter().enumerate() {
            let offset = node.start_byte() + 1;
            assert_eq!(
                find_gra_relation_index_at_offset(&tree, offset),
                Some(idx),
                "Failed for node {} at offset {}",
                idx,
                offset
            );
        }
        Ok(())
    }
}
