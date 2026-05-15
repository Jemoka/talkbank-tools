//! Selection range provider for CHAT-aware smart expand-selection.
//!
//! Expands selection through CHAT structural levels:
//! word → main tier content → main tier line → utterance block → section → file.

use tower_lsp::lsp_types::*;

/// Build nested selection ranges for CHAT structure at cursor positions.
pub fn selection_range(document: &str, positions: &[Position]) -> Vec<SelectionRange> {
    positions
        .iter()
        .map(|pos| build_selection_range(document, *pos))
        .collect()
}

fn build_selection_range(document: &str, position: Position) -> SelectionRange {
    let lines: Vec<&str> = document.lines().collect();
    let line_idx = position.line as usize;

    if line_idx >= lines.len() {
        return point_range(position);
    }

    let line = lines[line_idx];

    // Level 1: Current word (find word boundaries around cursor).
    let col = position.character as usize;
    let word_range = find_word_range(line, col, position.line);

    // Level 2: Line content (after the tab delimiter, if any).
    let content_range = find_content_range(line, position.line);

    // Level 3: Full current line.
    let line_range = Range {
        start: Position {
            line: position.line,
            character: 0,
        },
        end: Position {
            line: position.line,
            character: line.len() as u32,
        },
    };

    // Level 4: Utterance block (main tier + dependent tiers).
    let block_range = find_utterance_block(lines.as_slice(), line_idx);

    // Level 5: Entire file.
    let file_end_line = lines.len().saturating_sub(1);
    let file_end_char = lines.last().map_or(0, |l| l.len()) as u32;
    let file_range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: file_end_line as u32,
            character: file_end_char,
        },
    };

    // Build nested chain from innermost to outermost.
    let file_sel = SelectionRange {
        range: file_range,
        parent: None,
    };
    let block_sel = SelectionRange {
        range: block_range,
        parent: Some(Box::new(file_sel)),
    };
    let line_sel = SelectionRange {
        range: line_range,
        parent: Some(Box::new(block_sel)),
    };
    let content_sel = SelectionRange {
        range: content_range,
        parent: Some(Box::new(line_sel)),
    };
    SelectionRange {
        range: word_range,
        parent: Some(Box::new(content_sel)),
    }
}

/// Find the word boundaries around the cursor column.
fn find_word_range(line: &str, col: usize, line_num: u32) -> Range {
    let bytes = line.as_bytes();
    let col = col.min(line.len());

    let start = (0..col)
        .rev()
        .find(|&i| bytes.get(i).is_none_or(|b| *b == b' ' || *b == b'\t'))
        .map_or(0, |i| i + 1);

    let end = (col..line.len())
        .find(|&i| bytes.get(i).is_none_or(|b| *b == b' ' || *b == b'\t'))
        .unwrap_or(line.len());

    Range {
        start: Position {
            line: line_num,
            character: start as u32,
        },
        end: Position {
            line: line_num,
            character: end as u32,
        },
    }
}

/// Find the content portion of a tier line (after the tab).
fn find_content_range(line: &str, line_num: u32) -> Range {
    let content_start = line.find('\t').map_or(0, |i| i + 1);
    Range {
        start: Position {
            line: line_num,
            character: content_start as u32,
        },
        end: Position {
            line: line_num,
            character: line.len() as u32,
        },
    }
}

/// Find the utterance block containing the given line.
/// A block starts at `*SPEAKER:` and extends through consecutive `%tier:` lines.
fn find_utterance_block(lines: &[&str], line_idx: usize) -> Range {
    // Walk backwards to find the start of the block.
    let mut block_start = line_idx;
    for i in (0..=line_idx).rev() {
        if lines[i].starts_with('*') {
            block_start = i;
            break;
        }
        if lines[i].starts_with('@') && i < line_idx {
            block_start = line_idx;
            break;
        }
    }

    // Walk forward to find the end (until next *SPEAKER:, @header, or EOF).
    let mut block_end = line_idx;
    for (i, line) in lines.iter().enumerate().skip(block_start + 1) {
        if line.starts_with('*') || line.starts_with('@') {
            break;
        }
        block_end = i;
    }
    if block_end < block_start {
        block_end = block_start;
    }

    Range {
        start: Position {
            line: block_start as u32,
            character: 0,
        },
        end: Position {
            line: block_end as u32,
            character: lines.get(block_end).map_or(0, |l| l.len()) as u32,
        },
    }
}

fn point_range(position: Position) -> SelectionRange {
    SelectionRange {
        range: Range {
            start: position,
            end: position,
        },
        parent: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_range_nesting() {
        let doc = "@UTF8\n@Begin\n@Participants:\tCHI Child\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n@End\n";
        let pos = Position {
            line: 3,
            character: 8,
        }; // on "hello"
        let ranges = selection_range(doc, &[pos]);
        assert_eq!(ranges.len(), 1);

        // Innermost should be word-level range.
        let sel = &ranges[0];
        assert_eq!(sel.range.start.character, 6); // after tab
        assert_eq!(sel.range.end.character, 11); // "hello"

        // Should have nested parents.
        assert!(sel.parent.is_some());
        let content = sel.parent.as_ref().unwrap();
        assert!(content.parent.is_some());
    }

    #[test]
    fn test_selection_range_header_line() {
        let doc = "@UTF8\n@Begin\n@End\n";
        let pos = Position {
            line: 0,
            character: 2,
        };
        let ranges = selection_range(doc, &[pos]);
        assert_eq!(ranges.len(), 1);
        // Should still produce nested ranges without panicking.
        assert!(ranges[0].parent.is_some());
    }
}
