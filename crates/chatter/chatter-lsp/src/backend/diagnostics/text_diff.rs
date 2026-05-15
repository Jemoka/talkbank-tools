//! Text diff and byte↔point conversion for incremental parsing.
//!
//! Computes the minimal changed byte range between old and new document text,
//! then converts it to tree-sitter `Range` / `Point` values so the incremental
//! CST edit and re-parse can focus on just the touched region.
/// Compute text changed range for tree-sitter incremental parsing.
///
/// Returns a tree-sitter Range covering the edited region *in the new text* so
/// incremental reparsing can focus on just the touched nodes.
pub fn compute_text_changed_range(old_text: &str, new_text: &str) -> Option<tree_sitter::Range> {
    let (start, _old_end, new_end) = compute_text_diff_span(old_text, new_text)?;

    Some(tree_sitter::Range {
        start_byte: start,
        end_byte: new_end,
        start_point: byte_to_point(new_text, start),
        end_point: byte_to_point(new_text, new_end),
    })
}

/// Compute the differing span between old and new text.
///
/// Emits `(start_byte, old_end_byte, new_end_byte)` so callers can adjust cached
/// spans after edits with minimal computation.
///
/// Returns (start_byte, old_end_byte, new_end_byte) of the changed region.
pub fn compute_text_diff_span(old_text: &str, new_text: &str) -> Option<(usize, usize, usize)> {
    if old_text == new_text {
        return None;
    }

    let old_bytes = old_text.as_bytes();
    let new_bytes = new_text.as_bytes();
    let mut start = 0;
    let min_len = old_bytes.len().min(new_bytes.len());
    while start < min_len && old_bytes[start] == new_bytes[start] {
        start += 1;
    }

    let mut old_end = old_bytes.len();
    let mut new_end = new_bytes.len();
    while old_end > start && new_end > start && old_bytes[old_end - 1] == new_bytes[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }

    Some((start, old_end, new_end))
}

/// Convert a byte offset to a tree-sitter Point (row, column).
///
/// Used to derive diagnostic positions when adjusting spans after incremental edits.
pub fn byte_to_point(text: &str, byte: usize) -> tree_sitter::Point {
    let mut row = 0;
    let mut column = 0;
    let mut count = 0;
    for ch in text.chars() {
        if count >= byte {
            break;
        }
        if ch == '\n' {
            row += 1;
            column = 0;
        } else {
            column += ch.len_utf16();
        }
        count += ch.len_utf8();
    }

    tree_sitter::Point { row, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_span_returns_none_for_identical_text() {
        assert_eq!(compute_text_diff_span("hello", "hello"), None);
    }

    #[test]
    fn diff_span_detects_single_insertion() {
        assert_eq!(compute_text_diff_span("abc", "abXc"), Some((2, 2, 3)),);
    }

    #[test]
    fn diff_span_detects_middle_replacement() {
        assert_eq!(
            compute_text_diff_span("hello world", "hello rust"),
            Some((6, 11, 10)),
        );
    }

    #[test]
    fn changed_range_uses_new_text_coordinates() {
        let range = compute_text_changed_range("abc", "abXc").expect("changed range");
        assert_eq!(range.start_byte, 2);
        assert_eq!(range.end_byte, 3);
        assert_eq!(range.start_point.row, 0);
        assert_eq!(range.start_point.column, 2);
        assert_eq!(range.end_point.row, 0);
        assert_eq!(range.end_point.column, 3);
    }

    #[test]
    fn byte_to_point_handles_multibyte_characters() {
        let text = "a\n你b";
        let point = byte_to_point(text, 5);
        assert_eq!(point.row, 1);
        assert_eq!(point.column, 1);
    }
}
