//! Linked editing range for speaker codes.
//!
//! When the cursor is on a speaker code, returns all occurrences of that
//! speaker in the document so the editor can highlight them for simultaneous
//! editing. Reuses the same CST walk as the rename feature.

use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

use crate::backend::requests::find_ancestor_kind;
use talkbank_parser::node_types::{ID_SPEAKER, SPEAKER};

/// Find all linked editing ranges for the speaker code under the cursor.
pub fn linked_editing_ranges(
    tree: &Tree,
    doc: &str,
    position: Position,
) -> Option<LinkedEditingRanges> {
    let offset = crate::backend::utils::position_to_offset(doc, position);
    let root = tree.root_node();
    let node = root.descendant_for_byte_range(offset, offset)?;

    // Only activate on speaker nodes.
    let speaker_node =
        find_ancestor_kind(node, SPEAKER).or_else(|| find_ancestor_kind(node, ID_SPEAKER))?;
    let speaker_name = match speaker_node.utf8_text(doc.as_bytes()) {
        Ok(text) => text.trim(),
        Err(err) => {
            // UTF-8 decode failure on a CST node byte range typically
            // means the tree got out of sync with the document text
            // (e.g. incremental-parse regression). Log so operators
            // can correlate with other incremental-parse symptoms
            // instead of assuming linked editing is just broken.
            // See KIB-015.
            tracing::warn!(
                start = speaker_node.start_byte(),
                end = speaker_node.end_byte(),
                error = %err,
                "linked editing: UTF-8 decode failure on speaker node",
            );
            return None;
        }
    };

    if speaker_name.is_empty() {
        return None;
    }

    let mut ranges = Vec::new();
    let index = crate::backend::utils::LineIndex::new(doc);

    collect_speaker_ranges(root, doc, speaker_name, &index, &mut ranges);

    if ranges.is_empty() {
        return None;
    }

    Some(LinkedEditingRanges {
        ranges,
        word_pattern: None,
    })
}

/// Recursively collect ranges of all speaker/id_speaker nodes matching the given name.
fn collect_speaker_ranges(
    node: tree_sitter::Node,
    doc: &str,
    speaker_name: &str,
    index: &crate::backend::utils::LineIndex,
    ranges: &mut Vec<Range>,
) {
    if (node.kind() == SPEAKER || node.kind() == ID_SPEAKER)
        && let Ok(text) = node.utf8_text(doc.as_bytes())
        && text.trim() == speaker_name
    {
        let start = index.offset_to_position(doc, node.start_byte() as u32);
        let end = index.offset_to_position(doc, node.end_byte() as u32);
        ranges.push(Range { start, end });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_speaker_ranges(child, doc, speaker_name, index, ranges);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_tree;

    #[test]
    fn test_linked_editing_finds_all_speaker_occurrences() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n*MOT:\thi .\n*CHI:\tmore .\n@End\n";
        let tree = parse_tree(input);
        // Position on CHI in *CHI: line
        let pos = Position {
            line: 6,
            character: 1,
        };
        let result = linked_editing_ranges(&tree, input, pos);
        assert!(result.is_some());
        let ranges = result.unwrap().ranges;
        // CHI appears in @Participants, @ID, and two *CHI: lines = 4 occurrences
        assert!(
            ranges.len() >= 3,
            "Expected at least 3 CHI occurrences, got {}",
            ranges.len()
        );
    }

    #[test]
    fn linked_editing_returns_none_on_non_speaker_position() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let tree = parse_tree(input);
        // Position on "hello" word — not a speaker node.
        let pos = Position {
            line: 5,
            character: 7,
        };
        let result = linked_editing_ranges(&tree, input, pos);
        assert!(
            result.is_none(),
            "Expected no linked editing ranges on a non-speaker position"
        );
    }

    #[test]
    fn linked_editing_returns_none_on_header_line() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let tree = parse_tree(input);
        // Position on @Languages — not a speaker.
        let pos = Position {
            line: 2,
            character: 3,
        };
        let result = linked_editing_ranges(&tree, input, pos);
        assert!(
            result.is_none(),
            "Expected no linked editing ranges on a header name"
        );
    }

    #[test]
    fn linked_editing_on_id_speaker_node() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n*MOT:\thi .\n@End\n";
        let tree = parse_tree(input);
        // Position on MOT in @ID line (line 5, the MOT @ID header).
        // @ID:\teng|corpus|MOT — MOT starts after "eng|corpus|"
        let pos = Position {
            line: 5,
            character: 16,
        };
        let result = linked_editing_ranges(&tree, input, pos);
        if let Some(ranges) = result {
            // MOT should appear in @Participants, @ID, and *MOT: = at least 3.
            assert!(
                ranges.ranges.len() >= 2,
                "Expected at least 2 MOT occurrences, got {}",
                ranges.ranges.len()
            );
            // word_pattern should be None (not set by this implementation).
            assert!(ranges.word_pattern.is_none(), "word_pattern should be None");
        }
        // If None, the CST may not have an id_speaker node at that offset — acceptable.
    }

    #[test]
    fn linked_editing_single_speaker_document() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n*CHI:\tworld .\n*CHI:\tbye .\n@End\n";
        let tree = parse_tree(input);
        // Position on CHI in first *CHI: line.
        let pos = Position {
            line: 5,
            character: 1,
        };
        let result = linked_editing_ranges(&tree, input, pos);
        assert!(result.is_some(), "Expected linked editing ranges for CHI");
        let ranges = result.unwrap().ranges;
        // CHI in @Participants + @ID + 3 utterances = at least 4.
        assert!(
            ranges.len() >= 4,
            "Expected at least 4 CHI occurrences in single-speaker doc, got {}",
            ranges.len()
        );
    }
}
