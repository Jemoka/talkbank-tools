//! Find all references for speaker codes.
//!
//! Implements `textDocument/references` — when the cursor is on a speaker code,
//! returns all locations where that speaker code appears in the document:
//! - `@Participants` header entries
//! - `@ID` header speaker fields
//! - `*SPEAKER:` main tier prefixes
//! - `@Birth of SPEAKER`, `@L1 of SPEAKER` headers

use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

use super::super::utils;
use crate::backend::requests::{find_ancestor_kind, walk_nodes};
use talkbank_parser::node_types::{
    BIRTH_OF_HEADER, BIRTHPLACE_OF_HEADER, ID_SPEAKER, L1_OF_HEADER, SPEAKER,
};

/// Finds all references to the speaker code at the given position.
pub fn references(
    tree: &Tree,
    uri: &Url,
    doc: &str,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let speaker_name = find_speaker_name_at_position(tree, doc, position)?;
    let locations = collect_speaker_locations(tree, uri, doc, &speaker_name, include_declaration);
    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

/// Extracts the speaker code text at the given position.
fn find_speaker_name_at_position(tree: &Tree, doc: &str, position: Position) -> Option<String> {
    let offset = utils::position_to_offset(doc, position);
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;

    if let Some(speaker_node) = find_ancestor_kind(node, SPEAKER) {
        return speaker_node
            .utf8_text(doc.as_bytes())
            .ok()
            .map(String::from);
    }

    if let Some(id_speaker_node) = find_ancestor_kind(node, ID_SPEAKER) {
        return id_speaker_node
            .utf8_text(doc.as_bytes())
            .ok()
            .map(String::from);
    }

    // Check @Birth of / @L1 of headers
    if node.kind() == SPEAKER
        && let Some(parent) = node.parent()
        && (parent.kind() == BIRTH_OF_HEADER
            || parent.kind() == L1_OF_HEADER
            || parent.kind() == BIRTHPLACE_OF_HEADER)
    {
        return node.utf8_text(doc.as_bytes()).ok().map(String::from);
    }

    None
}

/// Collects all locations of a speaker code in the document.
fn collect_speaker_locations(
    tree: &Tree,
    uri: &Url,
    doc: &str,
    speaker_name: &str,
    _include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();
    let root = tree.root_node();

    walk_nodes(root, |node| {
        let matches = match node.kind() {
            SPEAKER | ID_SPEAKER => node
                .utf8_text(doc.as_bytes())
                .ok()
                .is_some_and(|t| t == speaker_name),
            _ => false,
        };

        if matches {
            locations.push(Location {
                uri: uri.clone(),
                range: Range {
                    start: utils::offset_to_position(doc, node.start_byte() as u32),
                    end: utils::offset_to_position(doc, node.end_byte() as u32),
                },
            });
        }

        None::<()>
    });

    locations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_tree_incremental as parse_tree;

    const PREAMBLE: &str =
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n";

    #[test]
    fn test_references_finds_all_speaker_occurrences() {
        let input = format!(
            "{PREAMBLE}@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n*CHI:\tmore .\n@End\n"
        );
        let tree = parse_tree(&input);
        let uri = Url::parse("file:///test.cha").unwrap();

        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let refs = references(&tree, &uri, &input, Position { line, character: 1 }, true);
        assert!(refs.is_some());
        // @Participants CHI + @ID CHI + 2x *CHI: = 4
        assert_eq!(refs.unwrap().len(), 4);
    }

    #[test]
    fn test_references_returns_none_for_non_speaker() {
        let input = format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let tree = parse_tree(&input);
        let uri = Url::parse("file:///test.cha").unwrap();

        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let refs = references(&tree, &uri, &input, Position { line, character: 6 }, true);
        assert!(refs.is_none());
    }
}
