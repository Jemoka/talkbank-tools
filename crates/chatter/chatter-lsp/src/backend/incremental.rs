//! Incremental parsing — re-parse only affected utterances on edits.
//!
//! CHAT files are line-structured: headers (`@Header:…`), then utterances
//! (main tier `*SPK:…` plus dependent tiers `%mor:…`). This module exploits
//! that structure so that a typical 1–3 line edit only re-parses the enclosing
//! utterance rather than the entire file (~100–1000× faster).
//!
//! # Architecture
//!
//! [`IncrementalChatDocument`] caches the source text, parsed `ChatFile`,
//! tree-sitter CST, and line↔utterance mappings. On each `didChange`:
//!
//! 1. Apply the text edit and get tree-sitter's `changed_ranges`.
//! 2. [`collect_utterances_and_header_changes`] classifies whether a context-
//!    affecting header (Participants, Languages, Options, ID) was touched —
//!    requiring a full re-validate — or only decorative headers changed.
//! 3. [`detect_utterance_splice`] detects single utterance insertion / deletion
//!    for O(1) array splice instead of O(n) rebuild.
//! 4. [`affected_utterance_indices`] finds which existing utterances overlap the
//!    changed ranges and need re-parsing.

use std::ops::Range;
use tree_sitter::{Node, Range as TsRange, Tree};

use talkbank_model::model::ChatFile;

use super::line_offsets::{compute_line_offsets, find_line_for_offset};

/// Cached document with incremental parsing support.
///
/// Stores the parsed ChatFile, tree-sitter Tree, and line mappings
/// needed to efficiently re-parse only affected regions on edit.
#[derive(Debug)]
#[allow(dead_code)]
pub struct IncrementalChatDocument {
    /// Source text (owned for safety during async operations)
    text: String,

    /// Parsed CHAT file (may contain errors)
    chat_file: ChatFile,

    /// Tree-sitter parse tree (for incremental re-parsing)
    tree: Tree,

    /// Mapping from line numbers to utterance indices.
    /// `line_to_utterance[line_num]` = Some(utterance_idx) if the line
    /// is part of an utterance, None if it's a header or blank.
    line_to_utterance: Vec<Option<usize>>,

    /// Byte offset of each line start.
    /// `line_offsets[i]` = byte offset where line `i` begins.
    line_offsets: Vec<usize>,
}

#[allow(dead_code)]
impl IncrementalChatDocument {
    /// Create a new incremental document from parsed data.
    pub fn new(text: String, chat_file: ChatFile, tree: Tree) -> Self {
        let line_offsets = compute_line_offsets(&text);
        let line_to_utterance = build_line_utterance_map(&chat_file, &line_offsets);

        Self {
            text,
            chat_file,
            tree,
            line_to_utterance,
            line_offsets,
        }
    }

    /// Get the current source text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the parsed ChatFile.
    pub fn chat_file(&self) -> &ChatFile {
        &self.chat_file
    }

    /// Get the tree-sitter Tree for incremental parsing.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Get the total number of lines.
    pub fn line_count(&self) -> usize {
        self.line_offsets.len()
    }

    /// Convert a byte offset to a line number.
    pub fn offset_to_line(&self, offset: usize) -> usize {
        // Binary search for the line containing this offset
        match self.line_offsets.binary_search(&offset) {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        }
    }

    /// Convert a line number to byte offset.
    pub fn line_to_offset(&self, line: usize) -> Option<usize> {
        self.line_offsets.get(line).copied()
    }

    /// Get the utterance index for a given line, if any.
    pub fn utterance_at_line(&self, line: usize) -> Option<usize> {
        self.line_to_utterance.get(line).copied().flatten()
    }

    /// Find all utterances affected by an edit in the given byte range.
    ///
    /// Returns a range of utterance indices that need re-parsing.
    pub fn affected_utterances(&self, edit_range: Range<usize>) -> Range<usize> {
        let start_line = self.offset_to_line(edit_range.start);
        let end_line = self.offset_to_line(edit_range.end);

        // Find first affected utterance
        let mut first_affected = None;
        let mut last_affected = None;

        for line in start_line..=end_line.min(self.line_to_utterance.len().saturating_sub(1)) {
            if let Some(utt_idx) = self.utterance_at_line(line) {
                if first_affected.is_none() {
                    first_affected = Some(utt_idx);
                }
                last_affected = Some(utt_idx);
            }
        }

        match (first_affected, last_affected) {
            (Some(first), Some(last)) => first..last + 1,
            _ => 0..0, // No utterances affected (edit in headers)
        }
    }

    /// Apply a text edit and update the document.
    ///
    /// This is the main incremental update entry point.
    /// Returns true if the update was successful.
    pub fn apply_edit(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        new_tree: Tree,
        new_chat_file: ChatFile,
    ) {
        // Calculate the byte offset change
        let old_len = range.end - range.start;
        let new_len = new_text.len();
        let delta = new_len as isize - old_len as isize;

        // Update the source text
        self.text.replace_range(range.clone(), new_text);

        // Update tree and chat file
        self.tree = new_tree;
        self.chat_file = new_chat_file;

        // Rebuild line mappings (could be optimized for small edits)
        self.line_offsets = compute_line_offsets(&self.text);
        self.line_to_utterance = build_line_utterance_map(&self.chat_file, &self.line_offsets);

        // Note: For future optimization, we could update line_offsets incrementally
        // by adjusting offsets after the edit point by `delta`, and only rebuilding
        // line_to_utterance for affected lines. For now, full rebuild is simple and fast.
        let _ = delta; // Silence unused warning - will use in optimized version
    }

    /// Replace the entire document (full re-parse).
    ///
    /// Used when incremental parsing isn't beneficial (e.g., large edits).
    pub fn replace(&mut self, text: String, chat_file: ChatFile, tree: Tree) {
        self.text = text;
        self.chat_file = chat_file;
        self.tree = tree;
        self.line_offsets = compute_line_offsets(&self.text);
        self.line_to_utterance = build_line_utterance_map(&self.chat_file, &self.line_offsets);
    }
}

/// Returns whether header kind.
fn is_header_kind(kind: &str) -> bool {
    use talkbank_parser::node_types::{
        ACTIVITIES_HEADER, BCK_HEADER, BG_HEADER, BIRTH_OF_HEADER, BIRTHPLACE_OF_HEADER,
        BLANK_HEADER, COLOR_WORDS_HEADER, COMMENT_HEADER, DATE_HEADER, EG_HEADER, FONT_HEADER,
        G_HEADER, HEADER, ID_HEADER, L1_OF_HEADER, LANGUAGES_HEADER, LOCATION_HEADER, MEDIA_HEADER,
        NEW_EPISODE_HEADER, NUMBER_HEADER, OPTIONS_HEADER, PAGE_HEADER, PARTICIPANTS_HEADER,
        PID_HEADER, PRE_BEGIN_HEADER, RECORDING_QUALITY_HEADER, ROOM_LAYOUT_HEADER,
        SITUATION_HEADER, T_HEADER, TAPE_LOCATION_HEADER, TIME_DURATION_HEADER, TIME_START_HEADER,
        TRANSCRIBER_HEADER, TRANSCRIPTION_HEADER, TYPES_HEADER, VIDEOS_HEADER, WARNING_HEADER,
        WINDOW_HEADER,
    };

    matches!(
        kind,
        HEADER
            | PRE_BEGIN_HEADER
            | ACTIVITIES_HEADER
            | BCK_HEADER
            | BG_HEADER
            | BIRTH_OF_HEADER
            | BIRTHPLACE_OF_HEADER
            | BLANK_HEADER
            | COLOR_WORDS_HEADER
            | COMMENT_HEADER
            | DATE_HEADER
            | EG_HEADER
            | FONT_HEADER
            | G_HEADER
            | ID_HEADER
            | L1_OF_HEADER
            | LANGUAGES_HEADER
            | LOCATION_HEADER
            | MEDIA_HEADER
            | NEW_EPISODE_HEADER
            | NUMBER_HEADER
            | OPTIONS_HEADER
            | PAGE_HEADER
            | PARTICIPANTS_HEADER
            | PID_HEADER
            | RECORDING_QUALITY_HEADER
            | ROOM_LAYOUT_HEADER
            | SITUATION_HEADER
            | T_HEADER
            | TAPE_LOCATION_HEADER
            | TIME_DURATION_HEADER
            | TIME_START_HEADER
            | TRANSCRIBER_HEADER
            | TRANSCRIPTION_HEADER
            | TYPES_HEADER
            | VIDEOS_HEADER
            | WARNING_HEADER
            | WINDOW_HEADER
    )
}

/// Return whether two half-open byte ranges overlap.
fn ranges_overlap(start_a: usize, end_a: usize, start_b: usize, end_b: usize) -> bool {
    start_a < end_b && start_b < end_a
}

/// Whether a header kind affects validation context (participants, languages, options).
///
/// Changes to context-affecting headers require full validation context rebuild.
/// Changes to decorative headers (Comment, Date, Location, etc.) only need
/// header error re-validation, not utterance re-validation.
fn is_context_affecting_header(kind: &str) -> bool {
    use talkbank_parser::node_types::{
        ID_HEADER, LANGUAGES_HEADER, OPTIONS_HEADER, PARTICIPANTS_HEADER,
    };

    matches!(
        kind,
        PARTICIPANTS_HEADER | ID_HEADER | LANGUAGES_HEADER | OPTIONS_HEADER
    )
}

/// Collect utterance CST nodes in order and detect header changes.
///
/// Returns `(utterances, context_header_changed, any_header_changed)`:
/// - `context_header_changed`: a header that affects validation context was modified
///   (Participants, ID, Languages, Options)
/// - `any_header_changed`: any header at all was modified (including decorative ones
///   like Comment, Date, Location)
pub fn collect_utterances_and_header_changes<'a>(
    tree: &'a Tree,
    changed_ranges: &[TsRange],
) -> (Vec<Node<'a>>, bool, bool) {
    let mut utterances = Vec::new();
    // (start_byte, end_byte, context_affecting)
    let mut header_ranges: Vec<(usize, usize, bool)> = Vec::new();

    let root = tree.root_node();
    // Grammar wraps content in a full_document node; descend into it if present.
    let doc_node = root
        .children(&mut root.walk())
        .find(|c| c.kind() == talkbank_parser::node_types::FULL_DOCUMENT)
        .unwrap_or(root);
    let mut cursor = doc_node.walk();
    for child in doc_node.children(&mut cursor) {
        if child.is_missing() || child.is_error() {
            continue;
        }

        if child.kind() == talkbank_parser::node_types::LINE {
            let mut line_cursor = child.walk();
            for line_child in child.children(&mut line_cursor) {
                if line_child.is_missing() || line_child.is_error() {
                    continue;
                }

                let kind = line_child.kind();
                if kind == talkbank_parser::node_types::UTTERANCE {
                    utterances.push(line_child);
                } else if is_header_kind(kind) {
                    header_ranges.push((
                        line_child.start_byte(),
                        line_child.end_byte(),
                        is_context_affecting_header(kind),
                    ));
                }
            }
        } else if is_header_kind(child.kind()) {
            header_ranges.push((
                child.start_byte(),
                child.end_byte(),
                is_context_affecting_header(child.kind()),
            ));
        }
    }

    let mut context_header_changed = false;
    let mut any_header_changed = false;
    for range in changed_ranges {
        let start = range.start_byte;
        let end = range.end_byte;
        for &(h_start, h_end, context_affecting) in &header_ranges {
            if ranges_overlap(h_start, h_end, start, end) {
                any_header_changed = true;
                if context_affecting {
                    context_header_changed = true;
                }
            }
        }
    }

    (utterances, context_header_changed, any_header_changed)
}

/// Detect a single utterance insertion or deletion by comparing the new CST
/// utterance count against the old count and locating the splice point using
/// the byte position where the text edit begins.
///
/// `diff_start` is the first byte that differs between old and new text,
/// as computed by `compute_text_diff_span`.
///
/// Returns `Some((splice_idx, is_insertion))`:
/// - For insertion: `splice_idx` is the index in the **new** utterance array
/// - For deletion: `splice_idx` is the index in the **old** utterance array
///
/// Returns `None` if the count difference is not ±1.
pub fn detect_utterance_splice(
    utterance_nodes: &[Node],
    diff_start: usize,
    old_utterance_count: usize,
) -> Option<(usize, bool)> {
    let new_count = utterance_nodes.len();
    let diff = new_count as i64 - old_utterance_count as i64;
    if diff.abs() != 1 {
        return None;
    }

    if diff == 1 {
        // Insertion: find the new utterance that contains or starts at diff_start.
        // Note: end_byte() is exclusive, so use strict < for the upper bound.
        for (i, node) in utterance_nodes.iter().enumerate() {
            if node.start_byte() <= diff_start && diff_start < node.end_byte() {
                return Some((i, true));
            }
            if node.start_byte() > diff_start {
                return Some((i, true));
            }
        }
        // Inserted at the very end
        Some((new_count - 1, true))
    } else {
        // Deletion: the splice point is the first gap in the new array at or
        // after diff_start — i.e., the index where the deleted utterance was.
        let idx = utterance_nodes
            .iter()
            .position(|n| n.start_byte() >= diff_start)
            .unwrap_or(new_count);
        Some((idx, false))
    }
}

/// Find utterance indices whose CST nodes overlap changed ranges.
pub fn affected_utterance_indices<'a>(
    utterance_nodes: &[Node<'a>],
    changed_ranges: &[TsRange],
) -> Vec<usize> {
    if changed_ranges.is_empty() {
        return Vec::new();
    }

    let mut indices = Vec::new();
    for (idx, node) in utterance_nodes.iter().enumerate() {
        let start = node.start_byte();
        let end = node.end_byte();
        if changed_ranges
            .iter()
            .any(|range| ranges_overlap(start, end, range.start_byte, range.end_byte))
        {
            indices.push(idx);
        }
    }
    indices
}

/// Collect line indices for utterances in a ChatFile (in utterance order).
pub fn collect_utterance_line_indices(chat_file: &ChatFile) -> Vec<usize> {
    let mut indices = Vec::new();
    for (idx, line) in chat_file.lines.iter().enumerate() {
        if matches!(line, talkbank_model::model::Line::Utterance(_)) {
            indices.push(idx);
        }
    }
    indices
}

/// Build a mapping from line numbers to utterance indices.
#[allow(dead_code)]
fn build_line_utterance_map(chat_file: &ChatFile, line_offsets: &[usize]) -> Vec<Option<usize>> {
    use talkbank_model::model::Line;

    let mut map = vec![None; line_offsets.len()];
    let mut current_utterance_idx = 0;

    for line in chat_file.lines.iter() {
        match line {
            Line::Utterance(utterance) => {
                // Find the line number for this utterance's start byte offset
                let start_offset = utterance.main.span.start as usize;
                let start_line = find_line_for_offset(line_offsets, start_offset);

                // Find end line (last line of the utterance)
                // Note: Using main tier span end for simplicity. Dependent tiers
                // are on subsequent lines but for mapping purposes this is sufficient.
                let end_offset = utterance.main.span.end as usize;
                let end_line = find_line_for_offset(line_offsets, end_offset);

                // Mark all lines in this utterance
                for line_num in start_line..=end_line.min(map.len().saturating_sub(1)) {
                    map[line_num] = Some(current_utterance_idx);
                }

                current_utterance_idx += 1;
            }
            Line::Header { .. } => {
                // Headers don't belong to utterances
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compute a tree-sitter InputEdit from an old and new text by finding the
    /// minimal differing region. This mirrors what the LSP does when it receives
    /// a `textDocument/didChange` notification and must edit the cached tree.
    fn compute_input_edit(old_text: &str, new_text: &str) -> tree_sitter::InputEdit {
        let start_byte = old_text
            .bytes()
            .zip(new_text.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(old_text.len().min(new_text.len()));

        let old_remaining = old_text.len() - start_byte;
        let new_remaining = new_text.len() - start_byte;
        let common_suffix = old_text
            .bytes()
            .rev()
            .zip(new_text.bytes().rev())
            .take_while(|(a, b)| a == b)
            .count()
            .min(old_remaining)
            .min(new_remaining);

        let old_end_byte = old_text.len() - common_suffix;
        let new_end_byte = new_text.len() - common_suffix;

        tree_sitter::InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position: byte_to_point(old_text, start_byte),
            old_end_position: byte_to_point(old_text, old_end_byte),
            new_end_position: byte_to_point(new_text, new_end_byte),
        }
    }

    /// Convert byte offset in UTF-8 text to tree-sitter `(row, column)` point.
    fn byte_to_point(text: &str, byte: usize) -> tree_sitter::Point {
        let prefix = &text[..byte];
        let row = prefix.bytes().filter(|&b| b == b'\n').count();
        let col = prefix.len() - prefix.rfind('\n').map(|p| p + 1).unwrap_or(0);
        tree_sitter::Point { row, column: col }
    }

    /// Parse old_text, apply the edit, parse new_text incrementally,
    /// and return (old_tree, new_tree) with correct changed_ranges.
    fn incremental_parse(old_text: &str, new_text: &str) -> (tree_sitter::Tree, tree_sitter::Tree) {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_talkbank::LANGUAGE.into())
            .expect("failed to set tree-sitter language");

        let mut old_tree = parser
            .parse(old_text, None)
            .expect("failed to parse old_text");

        let edit = compute_input_edit(old_text, new_text);
        old_tree.edit(&edit);

        let new_tree = parser
            .parse(new_text, Some(&old_tree))
            .expect("failed to parse new_text incrementally");

        (old_tree, new_tree)
    }

    /// Find the first byte position where old and new text differ.
    fn compute_diff_start(old_text: &str, new_text: &str) -> usize {
        old_text
            .bytes()
            .zip(new_text.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(old_text.len().min(new_text.len()))
    }

    // Tests for `compute_line_offsets` and `find_line_for_offset`
    // now live next to the implementation in `backend/line_offsets.rs`.

    /// Minimal valid CHAT preamble for splice tests.
    const PREAMBLE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|test|CHI|||||Child|||\n";

    #[test]
    fn test_detect_splice_insertion_at_end() {
        let old_text = &format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\thello .\n*CHI:\tworld .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _, _) =
            collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        assert_eq!(new_utterances.len(), 2);
        let result = detect_utterance_splice(&new_utterances, diff_start, 1);
        assert_eq!(result, Some((1, true))); // insertion at index 1
    }

    #[test]
    fn test_detect_splice_insertion_at_front() {
        let old_text = &format!("{PREAMBLE}*CHI:\tworld .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\thello .\n*CHI:\tworld .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _, _) =
            collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        assert_eq!(new_utterances.len(), 2);
        let result = detect_utterance_splice(&new_utterances, diff_start, 1);
        assert_eq!(result, Some((0, true))); // insertion at index 0
    }

    #[test]
    fn test_detect_splice_deletion() {
        let old_text = &format!("{PREAMBLE}*CHI:\thello .\n*CHI:\tworld .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\thello .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _, _) =
            collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        assert_eq!(new_utterances.len(), 1);
        let result = detect_utterance_splice(&new_utterances, diff_start, 2);
        assert_eq!(result, Some((1, false))); // deletion at old index 1
    }

    #[test]
    fn test_detect_splice_count_diff_too_large() {
        let old_text = &format!("{PREAMBLE}*CHI:\thello .\n@End\n");
        let new_text = &format!("{PREAMBLE}*CHI:\ta .\n*CHI:\tb .\n*CHI:\tc .\n@End\n");

        let (old_tree, new_tree) = incremental_parse(old_text, new_text);
        let diff_start = compute_diff_start(old_text, new_text);

        let changed_ranges: Vec<TsRange> = old_tree.changed_ranges(&new_tree).collect();
        let (new_utterances, _, _) =
            collect_utterances_and_header_changes(&new_tree, &changed_ranges);

        // Count diff is +2, not ±1 — should return None
        let result = detect_utterance_splice(&new_utterances, diff_start, 1);
        assert_eq!(result, None);
    }
}
