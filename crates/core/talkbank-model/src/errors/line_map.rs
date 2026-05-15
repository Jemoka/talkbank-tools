//! Byte-oriented line index used by parser and validator diagnostics.
//!
//! `LineMap` is the shared bridge between byte spans (`Span`, `SourceLocation`)
//! and human-facing line/column coordinates.
//!
//! ## Coordinate model
//!
//! - Input offsets are interpreted as UTF-8 byte offsets.
//! - Returned `(line, column)` pairs are 0-indexed.
//! - A byte offset on a newline byte belongs to the preceding line.
//! - Out-of-range offsets clamp to the last known line.
//!
//! This keeps line lookups deterministic and avoids repeated O(n) scans over
//! source text in every diagnostic path.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
/// Immutable index from byte offsets to line/column coordinates.
///
/// Stores the byte offset of the start of each line. Construction is a single
/// O(n) pass over the source; lookups are O(log n) via binary search.
///
/// All line/column values returned are **0-indexed**. Callers that need
/// 1-indexed values (e.g., for display) should add 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMap {
    /// Byte offset of the start of each line. `line_starts[0] == 0` always.
    line_starts: Vec<u32>,
}

impl LineMap {
    /// Build a `LineMap` from source text. Single O(n) pass.
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Return the 0-indexed line for a byte offset.
    ///
    /// Uses O(log n) binary search over line starts.
    ///
    /// Offsets beyond the source length are clamped to the last line.
    #[inline]
    pub fn line_of(&self, byte_offset: u32) -> usize {
        // partition_point returns the first index where line_starts[i] > byte_offset,
        // so subtracting 1 gives us the line whose start is <= byte_offset.
        self.line_starts
            .partition_point(|&start| start <= byte_offset)
            .saturating_sub(1)
    }

    /// Return `(line, column)` for a byte offset.
    ///
    /// Both outputs are 0-indexed. Column is measured in bytes from the start
    /// of the resolved line, so multi-byte Unicode scalars advance by more
    /// than one column.
    #[inline]
    pub fn line_col_of(&self, byte_offset: u32) -> (usize, usize) {
        let line = self.line_of(byte_offset);
        let col = (byte_offset - self.line_starts[line]) as usize;
        (line, col)
    }

    /// Byte offset of a line's start. O(1).
    ///
    /// Panics if `line >= self.line_count()`.
    #[inline]
    pub fn line_start(&self, line: usize) -> u32 {
        self.line_starts[line]
    }

    /// Byte offset just past the end of a line (exclusive), clamped to `source_len`.
    ///
    /// For lines that end with `\n`, the returned offset is the byte *after* the `\n`.
    /// For the last line (or a line beyond the file), returns `source_len`.
    #[inline]
    pub fn line_end(&self, line: usize, source_len: u32) -> u32 {
        if line + 1 < self.line_starts.len() {
            self.line_starts[line + 1]
        } else {
            source_len
        }
    }

    /// Total number of lines (including a final empty line after trailing `\n`).
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty input still has one logical line at offset 0.
    #[test]
    fn empty_source() {
        let lm = LineMap::new("");
        assert_eq!(lm.line_count(), 1);
        assert_eq!(lm.line_of(0), 0);
        assert_eq!(lm.line_col_of(0), (0, 0));
        assert_eq!(lm.line_start(0), 0);
    }

    /// Single-line input without trailing newline.
    #[test]
    fn single_line_no_newline() {
        let lm = LineMap::new("hello");
        assert_eq!(lm.line_count(), 1);
        assert_eq!(lm.line_of(0), 0);
        assert_eq!(lm.line_of(4), 0);
        assert_eq!(lm.line_col_of(3), (0, 3));
    }

    /// A trailing newline creates an extra empty terminal line.
    #[test]
    fn single_line_with_newline() {
        let lm = LineMap::new("hello\n");
        assert_eq!(lm.line_count(), 2);
        assert_eq!(lm.line_of(0), 0); // 'h'
        assert_eq!(lm.line_of(5), 0); // '\n' at offset 5
        assert_eq!(lm.line_of(6), 1); // start of empty second line
    }

    /// Multi-line mapping across several newline boundaries.
    #[test]
    fn multi_line() {
        let src = "abc\ndef\nghi\n";
        let lm = LineMap::new(src);
        assert_eq!(lm.line_count(), 4); // 3 lines + trailing empty

        // Line 0: "abc\n" at offsets 0..4
        assert_eq!(lm.line_of(0), 0);
        assert_eq!(lm.line_of(2), 0); // 'c'
        assert_eq!(lm.line_col_of(2), (0, 2));

        // Line 1: "def\n" at offsets 4..8
        assert_eq!(lm.line_of(4), 1);
        assert_eq!(lm.line_of(6), 1); // 'f'
        assert_eq!(lm.line_col_of(6), (1, 2));

        // Line 2: "ghi\n" at offsets 8..12
        assert_eq!(lm.line_of(8), 2);
        assert_eq!(lm.line_col_of(10), (2, 2));

        // Line 3: empty trailing line at offset 12
        assert_eq!(lm.line_of(12), 3);
        assert_eq!(lm.line_col_of(12), (3, 0));
    }

    /// Offsets on a newline byte map to the preceding line.
    #[test]
    fn offset_at_newline_boundary() {
        let src = "ab\ncd\n";
        let lm = LineMap::new(src);

        // '\n' at offset 2 belongs to line 0
        assert_eq!(lm.line_of(2), 0);
        // 'c' at offset 3 belongs to line 1
        assert_eq!(lm.line_of(3), 1);
    }

    /// UTF-8 code points are measured by bytes, not scalar index.
    #[test]
    fn utf8_content() {
        // "é" is 2 bytes (0xC3 0xA9), "日" is 3 bytes (0xE6 0x97 0xA5)
        let src = "é\n日\n";
        let lm = LineMap::new(src);
        assert_eq!(lm.line_count(), 3);

        // "é\n" = 3 bytes (offsets 0..3)
        assert_eq!(lm.line_of(0), 0);
        assert_eq!(lm.line_of(1), 0); // second byte of é
        assert_eq!(lm.line_of(2), 0); // '\n'

        // "日\n" = 4 bytes (offsets 3..7)
        assert_eq!(lm.line_of(3), 1);
        assert_eq!(lm.line_of(5), 1); // third byte of 日
        assert_eq!(lm.line_of(6), 1); // '\n'

        assert_eq!(lm.line_of(7), 2); // empty trailing line
    }

    /// Out-of-range offsets clamp to the final line.
    #[test]
    fn clamping_beyond_source() {
        let lm = LineMap::new("hi\n");
        // Offset far beyond source: should clamp to last line
        assert_eq!(lm.line_of(999), 1);
    }

    /// `line_start` matches expected per-line byte offsets.
    #[test]
    fn line_start_values() {
        let src = "ab\ncd\nef\n";
        let lm = LineMap::new(src);
        assert_eq!(lm.line_start(0), 0);
        assert_eq!(lm.line_start(1), 3);
        assert_eq!(lm.line_start(2), 6);
        assert_eq!(lm.line_start(3), 9);
    }

    /// Consecutive newlines produce consecutive empty lines.
    #[test]
    fn consecutive_newlines() {
        let src = "\n\n\n";
        let lm = LineMap::new(src);
        assert_eq!(lm.line_count(), 4);
        assert_eq!(lm.line_start(0), 0);
        assert_eq!(lm.line_start(1), 1);
        assert_eq!(lm.line_start(2), 2);
        assert_eq!(lm.line_start(3), 3);
    }
}
