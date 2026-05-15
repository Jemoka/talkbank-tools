//! On-type formatting for CHAT files.
//!
//! Auto-inserts a tab character after typing `:` when the line starts with
//! `*SPEAKER:` or `%tier:`, matching CHAT format conventions.

use tower_lsp::lsp_types::*;

/// Generate on-type formatting edits when `:` is typed on a tier line.
pub fn on_type_formatting(document: &str, position: Position, ch: &str) -> Option<Vec<TextEdit>> {
    if ch != ":" {
        return None;
    }

    let lines: Vec<&str> = document.lines().collect();
    let line_idx = position.line as usize;
    let line = lines.get(line_idx)?;

    // Only trigger on main tier (*SPEAKER:) or dependent tier (%tier:) lines.
    let is_main_tier = line.starts_with('*') && line.ends_with(':');
    let is_dep_tier = line.starts_with('%') && line.ends_with(':');

    if !is_main_tier && !is_dep_tier {
        return None;
    }

    // Don't insert if there's already a tab after the colon.
    if line.len() > position.character as usize
        && line.as_bytes()[position.character as usize] == b'\t'
    {
        return None;
    }

    let insert_pos = Position {
        line: position.line,
        character: position.character,
    };

    Some(vec![TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: "\t".to_string(),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_tab_after_main_tier() {
        let doc = "*CHI:";
        let pos = Position {
            line: 0,
            character: 5,
        };
        let edits = on_type_formatting(doc, pos, ":");
        assert!(edits.is_some());
        assert_eq!(edits.unwrap()[0].new_text, "\t");
    }

    #[test]
    fn test_insert_tab_after_dep_tier() {
        let doc = "%mor:";
        let pos = Position {
            line: 0,
            character: 5,
        };
        let edits = on_type_formatting(doc, pos, ":");
        assert!(edits.is_some());
    }

    #[test]
    fn test_no_insert_on_header() {
        let doc = "@Languages:";
        let pos = Position {
            line: 0,
            character: 11,
        };
        let edits = on_type_formatting(doc, pos, ":");
        assert!(edits.is_none());
    }

    #[test]
    fn test_no_insert_on_non_colon() {
        let doc = "*CHI:";
        let pos = Position {
            line: 0,
            character: 5,
        };
        let edits = on_type_formatting(doc, pos, "a");
        assert!(edits.is_none());
    }
}
