//! Source text helpers for semantic-diff rendering diagnostics.
//!
//! These helpers bridge byte-oriented spans to human-readable line/column
//! snippets so diff reports can point contributors directly to problematic
//! transcript locations.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::Span;

/// Source-line metadata extracted for one span start position.
///
/// Used by semantic-diff reports to attach human-readable line context to
/// byte-oriented spans.
pub(super) struct LineContext {
    pub line_num: usize,
    pub line_content: String,
}

/// Extracts line-number and full-line text for a span start offset.
pub(super) fn extract_line_context(source: &str, span: Span) -> Option<LineContext> {
    if span.is_dummy() {
        return None;
    }

    let start = span.start as usize;
    if start > source.len() {
        return None;
    }

    // Find line number and line start
    let mut line_num = 1usize;
    let mut line_start = 0usize;
    for (idx, ch) in source.char_indices() {
        if idx >= start {
            break;
        }
        if ch == '\n' {
            line_num += 1;
            line_start = idx + 1;
        }
    }

    // Extract the line
    let line_end = match source[line_start..].find('\n') {
        Some(idx) => line_start + idx,
        None => source.len(),
    };
    let line_content = source[line_start..line_end].to_string();

    Some(LineContext {
        line_num,
        line_content,
    })
}

/// Extract a specific 1-indexed line from source text.
///
/// Returns `None` when the requested line number is out of range.
pub(super) fn extract_line_by_number(source: &str, target_line: usize) -> Option<String> {
    source
        .lines()
        .nth(target_line.saturating_sub(1))
        .map(|s| s.to_string())
}

/// Find the byte range for a specific 1-indexed line in source text.
///
/// Returns `(start_byte, end_byte)` where `end_byte` is exclusive. The helper
/// is byte-accurate and does not normalize line endings.
#[allow(dead_code)]
pub(super) fn find_line_byte_range(source: &str, target_line: usize) -> Option<(usize, usize)> {
    if target_line == 0 || source.is_empty() {
        return None;
    }

    let mut current_line = 1usize;
    let mut line_start = 0usize;

    for (byte_idx, ch) in source.char_indices() {
        if ch == '\n' {
            if current_line == target_line {
                // Found end of target line
                return Some((line_start, byte_idx));
            }
            current_line += 1;
            line_start = byte_idx + 1;
        }
    }

    // Handle last line (no trailing newline)
    if current_line == target_line {
        return Some((line_start, source.len()));
    }

    None
}

/// Renderable span snippet with location, source line, and caret marker.
///
/// This keeps formatting concerns localized so report rendering code can stay
/// focused on diff semantics instead of source-layout details.
pub(super) struct SpanSnippet {
    pub location: String,
    pub line: String,
    pub caret: String,
}

/// Build a display-ready snippet (location + source line + caret) for one span.
///
/// The caret column is computed from byte offset to line/column conversion and
/// is intended for human diagnostics, not parser roundtrips.
pub(super) fn span_snippet(source: &str, span: Span) -> Option<SpanSnippet> {
    if span.is_dummy() {
        return None;
    }
    let start = span.start as usize;
    if start > source.len() {
        return None;
    }
    let (line_num, col_num, line_start) = byte_offset_to_line_col(source, start)?;
    let line_end = match source[line_start..].find('\n') {
        Some(idx) => line_start + idx,
        None => source.len(),
    };
    let line = source[line_start..line_end].to_string();
    let caret = build_caret(&line, col_num);
    Some(SpanSnippet {
        location: format!("line {}, col {}", line_num, col_num),
        line,
        caret,
    })
}

/// Converts byte offset to `(line, column, line_start_offset)` (1-indexed line/col).
fn byte_offset_to_line_col(source: &str, offset: usize) -> Option<(usize, usize, usize)> {
    let mut line = 1usize;
    let mut col = 1usize;
    let mut last_line_start = 0usize;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            return Some((line, col, last_line_start));
        }
        if ch == '\n' {
            line += 1;
            col = 1;
            last_line_start = idx + ch.len_utf8();
        } else {
            col += 1;
        }
    }
    if offset == source.len() {
        return Some((line, col, last_line_start));
    }
    None
}

/// Build a caret line aligned to `col`, preserving tab characters.
///
/// Tabs are retained as tabs so monospace terminal output lines up with the
/// displayed source line.
fn build_caret(line: &str, col: usize) -> String {
    let mut caret = String::new();
    for (current, ch) in (1usize..).zip(line.chars()) {
        if current >= col {
            break;
        }
        if ch == '\t' {
            caret.push('\t');
        } else {
            caret.push(' ');
        }
    }
    caret.push('^');
    caret
}
