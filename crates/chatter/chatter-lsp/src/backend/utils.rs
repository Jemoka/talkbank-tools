//! Offset ↔ LSP position conversion and utterance lookup helpers.
//!
//! Provides [`LineIndex`] for amortised O(log n) conversions when many spans from
//! the same document need to be mapped (e.g. diagnostic publishing), plus
//! standalone [`offset_to_position`] / [`position_to_offset`] functions for
//! one-off conversions. All conversions respect UTF-8 character boundaries and
//! use 0-indexed lines/characters per LSP convention.

use talkbank_model::Span;
use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::Utterance;
use tower_lsp::lsp_types::*;

// =============================================================================
// LineIndex — O(log n) byte-offset → LSP Position conversion
// =============================================================================

/// Pre-computed line start offsets for O(log n) offset-to-position conversion.
///
/// Construct once per document text, then call `offset_to_position` for each span.
/// This amortizes the O(n) line scan over many lookups.
pub struct LineIndex {
    /// Byte offset of the start of each line. Always starts with 0.
    line_starts: Vec<u32>,
}

impl LineIndex {
    /// Build a line index from document text. O(n) in text length.
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, byte) in text.as_bytes().iter().enumerate() {
            if *byte == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Convert byte offset to LSP Position using binary search. O(log n) per call.
    pub fn offset_to_position(&self, text: &str, offset: u32) -> Position {
        let offset_usize = offset as usize;

        // Bounds check: clamp to end of text
        if offset_usize >= text.len() {
            let last_line = self.line_starts.len().saturating_sub(1);
            let line_start = self.line_starts[last_line] as usize;
            let character = text[line_start..].chars().count() as u32;
            return Position {
                line: last_line as u32,
                character,
            };
        }

        // Binary search: find the last line_start <= offset
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line] as usize;

        // Count UTF-8 characters from line start to offset
        let character = text[line_start..offset_usize].chars().count() as u32;

        Position {
            line: line as u32,
            character,
        }
    }
}

// =============================================================================
// Standalone offset conversion (for single-call use)
// =============================================================================

/// Convert byte offset to LSP Position (line, character).
///
/// For single conversions this is fine. When converting many offsets from the same
/// document, prefer constructing a [`LineIndex`] and calling its method instead.
pub fn offset_to_position(text: &str, offset: u32) -> Position {
    let offset = offset as usize;

    // Bounds check: if offset is beyond text, return end position
    if offset >= text.len() {
        let line_count = text.lines().count().saturating_sub(1);
        let last_line = text.lines().nth(line_count).unwrap_or_default();
        // DEFAULT: When the document has no lines, treat the last line as empty.
        return Position {
            line: line_count as u32,
            character: last_line.chars().count() as u32,
        };
    }

    let mut line = 0;
    let mut line_start_byte = 0;

    for (byte_idx, ch) in text.char_indices() {
        if byte_idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start_byte = byte_idx + 1;
        }
    }

    // Count UTF-8 characters from line start to offset
    let line_text = &text[line_start_byte..offset];
    let character = line_text.chars().count() as u32;

    Position { line, character }
}

/// Convert LSP Position (line, character) to byte offset
///
/// # Arguments
/// * `text` - Full document text
/// * `position` - LSP position (0-indexed line, UTF-8 character count on line)
///
/// # Returns
/// Byte offset in the document. If position is out of bounds, returns text.len()
pub fn position_to_offset(text: &str, position: Position) -> usize {
    let target_line = position.line as usize;
    let target_char = position.character as usize;

    let mut current_line = 0;
    let mut line_start_byte = 0;

    for (byte_idx, ch) in text.char_indices() {
        // If we're on the target line, count characters
        if current_line == target_line {
            let chars_from_line_start = text[line_start_byte..byte_idx].chars().count();
            if chars_from_line_start >= target_char {
                return byte_idx;
            }
        }

        // Track newlines to find line boundaries
        if ch == '\n' {
            // If we're still on target line when we hit newline, position is at end of line
            if current_line == target_line {
                return byte_idx;
            }
            current_line += 1;
            line_start_byte = byte_idx + 1;
        }
    }

    // Position is beyond document end or at end of last line
    if current_line == target_line {
        // We're on the target line, find the offset
        let remaining_text = &text[line_start_byte..];
        let char_count = remaining_text.chars().count();
        if target_char <= char_count {
            // Count characters forward from line start
            for (char_idx, (byte_idx, _)) in remaining_text.char_indices().enumerate() {
                if char_idx >= target_char {
                    return line_start_byte + byte_idx;
                }
            }
            // Target is at end of line
            return text.len();
        }
    }

    // Position beyond document end
    text.len()
}

/// Finds utterance at position.
pub fn find_utterance_at_position<'a>(
    chat_file: &'a talkbank_model::model::ChatFile,
    position: Position,
    document: &str,
) -> Option<&'a talkbank_model::model::Utterance> {
    let offset = position_to_offset(document, position) as u32;

    chat_file
        .utterances()
        .find(|utterance| utterance_contains_offset(utterance, offset))
}

/// Returns whether the offset falls within the utterance main/dependent-tier spans.
fn utterance_contains_offset(utterance: &Utterance, offset: u32) -> bool {
    span_contains(utterance.main.span, offset)
        || utterance
            .dependent_tiers
            .iter()
            .any(|tier| dependent_tier_span(tier).is_some_and(|span| span_contains(span, offset)))
}

/// Returns the source span for a dependent-tier variant.
fn dependent_tier_span(tier: &DependentTier) -> Option<Span> {
    Some(tier.span())
}

/// Returns whether the offset is inside the span (inclusive bounds).
fn span_contains(span: Span, offset: u32) -> bool {
    offset >= span.start && offset <= span.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_and_standalone_offset_conversion_match() {
        let text = "ab\ncd\n你z";
        let index = LineIndex::new(text);

        // `offset_to_position` expects valid UTF-8 boundaries when in-range.
        for offset in [0_u32, 1, 2, 3, 4, 5, 6, 9, 10, 11] {
            let indexed = index.offset_to_position(text, offset);
            let standalone = offset_to_position(text, offset);
            assert_eq!(indexed, standalone, "offset={offset}");
        }
    }

    #[test]
    fn position_to_offset_handles_multibyte_characters() {
        let text = "a\n你z";
        let pos_after_chinese = Position {
            line: 1,
            character: 1,
        };
        let pos_after_z = Position {
            line: 1,
            character: 2,
        };

        assert_eq!(position_to_offset(text, pos_after_chinese), 5);
        assert_eq!(position_to_offset(text, pos_after_z), 6);
    }

    #[test]
    fn span_contains_is_inclusive() {
        let span = Span { start: 10, end: 20 };
        assert!(span_contains(span, 10));
        assert!(span_contains(span, 15));
        assert!(span_contains(span, 20));
        assert!(!span_contains(span, 9));
        assert!(!span_contains(span, 21));
    }
}
