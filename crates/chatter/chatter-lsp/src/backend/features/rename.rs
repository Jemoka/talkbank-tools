//! Speaker code rename support.
//!
//! Implements `textDocument/rename` and `textDocument/prepareRename` for CHAT
//! speaker codes. Renaming a speaker code updates all occurrences:
//! - `@Participants` header entries
//! - `@ID` header speaker fields
//! - `*SPEAKER:` main tier prefixes
//! - `@Birth of SPEAKER`, `@L1 of SPEAKER` headers

use std::collections::HashMap;

use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

use super::super::utils;
use crate::backend::requests::{find_ancestor_kind, walk_nodes};

use talkbank_parser::node_types::{
    BIRTH_OF_HEADER, BIRTHPLACE_OF_HEADER, ID_SPEAKER, L1_OF_HEADER, SPEAKER,
};

/// Validates that the cursor is on a renameable speaker code and returns
/// its range and current text. Used for `textDocument/prepareRename`.
pub fn prepare_rename(tree: &Tree, doc: &str, position: Position) -> Option<PrepareRenameResponse> {
    let (range, text) = find_speaker_at_position(tree, doc, position)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range,
        placeholder: text,
    })
}

/// Computes a `WorkspaceEdit` that renames all occurrences of the speaker code
/// at the cursor position to `new_name`.
pub fn rename(
    tree: &Tree,
    uri: &Url,
    doc: &str,
    position: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let (_range, old_name) = find_speaker_at_position(tree, doc, position)?;

    let edits = collect_speaker_edits(tree, doc, &old_name, new_name);
    if edits.is_empty() {
        return None;
    }

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

/// Finds the speaker code node at the given cursor position and returns its
/// LSP range and text content. Returns `None` if the cursor is not on a
/// speaker code.
fn find_speaker_at_position(tree: &Tree, doc: &str, position: Position) -> Option<(Range, String)> {
    let offset = utils::position_to_offset(doc, position);
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;

    // Check if we're on a SPEAKER node (main tier or @Participants)
    if let Some(speaker_node) = find_ancestor_kind(node, SPEAKER) {
        let text = speaker_node.utf8_text(doc.as_bytes()).ok()?;
        let range = node_to_range(speaker_node, doc);
        return Some((range, text.to_string()));
    }

    // Check if we're on an ID_SPEAKER node (@ID header)
    if let Some(id_speaker_node) = find_ancestor_kind(node, ID_SPEAKER) {
        let text = id_speaker_node.utf8_text(doc.as_bytes()).ok()?;
        let range = node_to_range(id_speaker_node, doc);
        return Some((range, text.to_string()));
    }

    // Check if we're on a speaker code in @Birth of / @L1 of headers
    if let Some(speaker_text) = find_of_header_speaker(node, doc) {
        let range = node_to_range(node, doc);
        return Some((range, speaker_text));
    }

    None
}

/// Checks if a node is the speaker code portion of a `@Birth of SPEAKER` or
/// `@L1 of SPEAKER` header. These headers have a prefix node followed by a
/// speaker node.
fn find_of_header_speaker(node: Node, doc: &str) -> Option<String> {
    // The speaker code in "@Birth of CHI" is a SPEAKER child of the header node
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

/// Collects all `TextEdit`s needed to rename `old_name` to `new_name` across
/// the entire document.
fn collect_speaker_edits(tree: &Tree, doc: &str, old_name: &str, new_name: &str) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let root = tree.root_node();

    walk_nodes(root, |node| {
        match node.kind() {
            // Main tier speaker codes: *CHI:
            SPEAKER | ID_SPEAKER => {
                if let Ok(text) = node.utf8_text(doc.as_bytes())
                    && text == old_name
                {
                    edits.push(TextEdit {
                        range: node_to_range(node, doc),
                        new_text: new_name.to_string(),
                    });
                }
            }
            _ => {}
        }
        None::<()>
    });

    edits
}

/// Converts a tree-sitter node to an LSP `Range`.
fn node_to_range(node: Node, doc: &str) -> Range {
    Range {
        start: utils::offset_to_position(doc, node.start_byte() as u32),
        end: utils::offset_to_position(doc, node.end_byte() as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_tree_incremental as parse_tree;

    /// Valid CHAT header preamble for tests.
    const PREAMBLE: &str =
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n";

    #[test]
    fn test_prepare_rename_on_main_tier_speaker() {
        let input = format!("{PREAMBLE}@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n");
        let tree = parse_tree(&input);

        // Position on CHI in *CHI:
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = prepare_rename(&tree, &input, Position { line, character: 1 });
        assert!(result.is_some());

        if let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) = result {
            assert_eq!(placeholder, "CHI");
        }
    }

    #[test]
    fn test_prepare_rename_not_on_speaker() {
        let input = format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let tree = parse_tree(&input);

        // Position on "hello" — not a speaker code
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = prepare_rename(&tree, &input, Position { line, character: 6 });
        assert!(result.is_none());
    }

    #[test]
    fn test_rename_speaker_all_locations() {
        let input = format!(
            "{PREAMBLE}@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n*MOT:\thi .\n*CHI:\tmore .\n@End\n"
        );
        let tree = parse_tree(&input);
        let uri = Url::parse("file:///test.cha").unwrap();

        // Rename CHI → KID from the main tier
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = rename(&tree, &uri, &input, Position { line, character: 1 }, "KID");
        assert!(result.is_some());

        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Should find: @Participants CHI, @ID CHI, *CHI: (twice) = 4 edits
        assert_eq!(
            edits.len(),
            4,
            "Expected 4 edits (participants + ID + 2 main tiers), got {}",
            edits.len()
        );

        // All edits should replace with "KID"
        for e in edits {
            assert_eq!(e.new_text, "KID");
        }
    }

    #[test]
    fn test_rename_does_not_touch_other_speakers() {
        let input = format!("{PREAMBLE}*CHI:\thello .\n*MOT:\thi .\n@End\n");
        let tree = parse_tree(&input);
        let uri = Url::parse("file:///test.cha").unwrap();

        // Rename CHI → KID
        let line = input.lines().position(|l| l.starts_with("*CHI:")).unwrap() as u32;
        let result = rename(&tree, &uri, &input, Position { line, character: 1 }, "KID");

        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Verify MOT is not in any edit range
        for e in edits {
            let start_offset = utils::position_to_offset(&input, e.range.start);
            let end_offset = utils::position_to_offset(&input, e.range.end);
            let replaced = &input[start_offset..end_offset];
            assert_eq!(
                replaced, "CHI",
                "Edit should only replace CHI, not '{}'",
                replaced
            );
        }
    }
}
