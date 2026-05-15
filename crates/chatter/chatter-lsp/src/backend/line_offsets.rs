//! Pure text-to-line-offset utilities.
//!
//! Byte-offset-of-line-start and line-for-byte-offset. Operates only
//! on raw document text, so it lives outside `incremental.rs` and any
//! consumer that needs to map a tree-sitter byte span to a line range
//! can reach it without pulling in the incremental-reparse machinery.

/// Compute the byte offset of the start of each line in `text`.
///
/// The returned vector always starts with `0` (the start of line 0)
/// and appends one entry per `'\n'` character, pointing at the byte
/// immediately after the newline. For a document of N lines the
/// vector has N entries; a trailing newline produces an extra
/// one-past-the-end entry, which is fine because
/// [`find_line_for_offset`] uses `binary_search` semantics.
pub(crate) fn compute_line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Find the 0-indexed line number containing `offset`.
///
/// Uses a `binary_search` against the line-start offsets produced by
/// [`compute_line_offsets`]. On an exact hit (the offset is a
/// line-start) the result is that line; otherwise the search returns
/// the insertion position and we subtract one to get the preceding
/// line. `saturating_sub(1)` keeps offset `0` pointing at line `0`.
pub(crate) fn find_line_for_offset(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line,
        Err(line) => line.saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_line_offsets_indexes_each_line_start() {
        let text = "abc\ndef\n\ngh";
        // Line starts: 0 ('a'), 4 ('d'), 8 ('\n' itself is empty line), 9 ('g').
        let offsets = compute_line_offsets(text);
        assert_eq!(offsets, vec![0, 4, 8, 9]);
    }

    #[test]
    fn compute_line_offsets_handles_empty_text() {
        let offsets = compute_line_offsets("");
        assert_eq!(offsets, vec![0]);
    }

    #[test]
    fn compute_line_offsets_handles_trailing_newline() {
        let offsets = compute_line_offsets("abc\n");
        // Trailing '\n' produces a one-past-the-end line-start entry.
        assert_eq!(offsets, vec![0, 4]);
    }

    #[test]
    fn find_line_for_offset_returns_correct_line() {
        let offsets = vec![0, 6, 12, 18];
        assert_eq!(find_line_for_offset(&offsets, 0), 0);
        assert_eq!(find_line_for_offset(&offsets, 3), 0);
        assert_eq!(find_line_for_offset(&offsets, 6), 1);
        assert_eq!(find_line_for_offset(&offsets, 10), 1);
        assert_eq!(find_line_for_offset(&offsets, 12), 2);
        assert_eq!(find_line_for_offset(&offsets, 17), 2);
    }
}
