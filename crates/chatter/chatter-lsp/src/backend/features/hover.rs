//! Hover cards combining model state with alignment information.
//!
//! Builds markdown hover content by calling into [`crate::alignment`] to resolve
//! the tier item under the cursor, then formats the result via
//! [`format_alignment_info`]. Also provides hover for headers (`@Media`,
//! `@Languages`, `@Date`, etc.) and timing bullets.

use crate::alignment::{find_alignment_hover_info, format_alignment_info};
use crate::backend::state::ParseState;
use tower_lsp::lsp_types::*;

/// Markdown footer appended to alignment-consuming hover cards when the
/// backend is serving a `ParseState::StaleBaseline` (KIB-013).
///
/// Vocabulary (`stale baseline`) deliberately mirrors the
/// [`ParseState::StaleBaseline`] identifier so the same term flows from
/// source code to `tracing::debug!` logs to the user-facing hover. The
/// `---` separator + blockquote give the marker its own visual
/// hierarchy in the rendered markdown without competing with primary
/// content.
const STALE_BASELINE_HOVER_FOOTER: &str =
    "\n\n---\n\n> ⚠ **Stale baseline** — alignment reflects the last successful parse.";

/// Build hover markdown for alignment-aware nodes at the requested position.
///
/// When `parse_state` is [`ParseState::StaleBaseline`], a short footer
/// is appended to alignment-consuming hover cards so the user can tell
/// at a glance that the alignment payload is from the last clean parse
/// rather than from the current (un-reparseable) document text.
pub fn hover(
    chat_file: &talkbank_model::model::ChatFile,
    tree: &tree_sitter::Tree,
    position: Position,
    document: &str,
    parse_state: ParseState,
) -> Option<Hover> {
    // Try alignment hover first (main tier words, dependent tier items).
    if let Some(alignment_info) = find_alignment_hover_info(chat_file, tree, position, document) {
        let mut hover_text = format_alignment_info(&alignment_info);
        if parse_state == ParseState::StaleBaseline {
            hover_text.push_str(STALE_BASELINE_HOVER_FOOTER);
        }
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_text,
            }),
            range: None,
        });
    }

    // Try header hover.
    if let Some(text) = header_hover(document, position) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: text,
            }),
            range: None,
        });
    }

    // Try timing bullet hover.
    if let Some(text) = bullet_hover(document, position) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: text,
            }),
            range: None,
        });
    }

    None
}

/// Hover for CHAT headers — shows documentation for the header type.
fn header_hover(document: &str, position: Position) -> Option<String> {
    let line = document.lines().nth(position.line as usize)?;
    if !line.starts_with('@') {
        return None;
    }

    let header_name = line.split(['\t', ':']).next()?;
    let desc = match header_name {
        "@UTF8" => "**@UTF8** — UTF-8 encoding declaration. Must be the first line.",
        "@Begin" => "**@Begin** — Marks the start of the transcript body.",
        "@End" => "**@End** — Marks the end of the transcript. Must be the last line.",
        "@Languages" => {
            "**@Languages** — ISO 639 language code(s) for the transcript.\n\nFormat: `@Languages:\teng` or `@Languages:\teng, spa`"
        }
        "@Participants" => {
            "**@Participants** — Declares all speakers in the transcript.\n\nFormat: `@Participants:\tCHI Target_Child, MOT Mother`"
        }
        "@ID" => {
            "**@ID** — Participant identification.\n\nFormat: `@ID:\tlanguage|corpus|code|age|sex|group|SES|role|education|custom|`\n\n| Field | Description |\n|-------|-------------|\n| 1 | Language |\n| 2 | Corpus name |\n| 3 | Speaker code |\n| 4 | Age (Y;M.D) |\n| 5 | Sex (male/female) |\n| 6 | Group |\n| 7 | SES |\n| 8 | Role |\n| 9 | Education |\n| 10 | Custom |"
        }
        "@Media" => {
            "**@Media** — Associated media file.\n\nFormat: `@Media:\tfilename, audio` or `@Media:\tfilename, video`"
        }
        "@Date" => "**@Date** — Recording date.\n\nFormat: `@Date:\tDD-MMM-YYYY`",
        "@Location" => "**@Location** — Where the recording took place.",
        "@Situation" => "**@Situation** — Description of the recording context.",
        "@Comment" => "**@Comment** — General comment or annotation.",
        "@Bg" => {
            "**@Bg** — Begin gem. Marks the start of an activity section.\n\nFormat: `@Bg:\tActivityName`"
        }
        "@Eg" => {
            "**@Eg** — End gem. Marks the end of an activity section.\n\nFormat: `@Eg:\tActivityName`"
        }
        "@Options" => {
            "**@Options** — Transcript options.\n\nValues: `CA` (conversation analysis), `multi` (multiple languages)"
        }
        "@Activities" => "**@Activities** — Activities occurring during the session.",
        "@Birth of" | "@Birthplace of" | "@L1 of" => {
            return Some(format!(
                "**{header_name}** — Per-participant metadata header."
            ));
        }
        _ => return None,
    };

    Some(desc.to_string())
}

/// Hover for timing bullets — shows formatted duration.
///
/// CHAT bullets use U+0015 (NAK) as the delimiter: `\x15NNN_NNN\x15`.
fn bullet_hover(document: &str, position: Position) -> Option<String> {
    let line = document.lines().nth(position.line as usize)?;
    let col = position.character as usize;

    const BULLET: char = '\u{0015}';

    // Find bullet start (search backwards from cursor for NAK).
    let mut start = None;
    for (idx, ch) in line.char_indices().rev() {
        if idx > col {
            continue;
        }
        if ch == BULLET {
            start = Some(idx);
            break;
        }
    }

    let start = start?;
    // Find bullet end (next NAK after start).
    let rest = &line[start + BULLET.len_utf8()..];
    let end_pos = rest.find(BULLET)?;
    let inner = &rest[..end_pos];

    // Parse NNN_NNN
    let parts: Vec<&str> = inner.split('_').collect();
    if parts.len() != 2 {
        return None;
    }
    let beg: u64 = parts[0].parse().ok()?;
    let end: u64 = parts[1].parse().ok()?;

    let beg_s = beg as f64 / 1000.0;
    let end_s = end as f64 / 1000.0;
    let dur = end_s - beg_s;

    Some(format!(
        "**Timing** — {:.3}s to {:.3}s (duration: {:.3}s)",
        beg_s, end_s, dur
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_hover_languages() {
        let doc = "@UTF8\n@Begin\n@Languages:\teng\n@End\n";
        let pos = Position {
            line: 2,
            character: 3,
        };
        let hover = header_hover(doc, pos);
        assert!(hover.is_some());
        assert!(hover.unwrap().contains("ISO 639"));
    }

    #[test]
    fn test_header_hover_non_header_line() {
        let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let pos = Position {
            line: 2,
            character: 3,
        };
        assert!(header_hover(doc, pos).is_none());
    }

    #[test]
    fn test_bullet_hover_duration() {
        let doc = "*CHI:\thello \x151000_2500\x15 .\n";
        let pos = Position {
            line: 0,
            character: 14,
        };
        let hover = bullet_hover(doc, pos);
        assert!(hover.is_some());
        let text = hover.unwrap();
        assert!(text.contains("1.000s to 2.500s"));
        assert!(text.contains("duration: 1.500s"));
    }

    /// Hovering an alignment-aware node under `StaleBaseline` appends the
    /// markdown footer; under `Clean` it does not (KIB-013).
    #[test]
    fn alignment_hover_appends_stale_baseline_footer() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n%mor:\tn|hello .\n%gra:\t1|0|ROOT 2|1|PUNCT\n@End\n";
        let chat_file = crate::test_fixtures::parse_chat_with_alignments(content);
        let tree = crate::test_fixtures::parse_tree(content);
        // Cursor on the main-tier word `hello` (line 5, char 7).
        let pos = Position {
            line: 5,
            character: 7,
        };

        let clean = hover(&chat_file, &tree, pos, content, ParseState::Clean)
            .expect("hover must resolve the main-tier word");
        let clean_text = match &clean.contents {
            HoverContents::Markup(m) => &m.value,
            _ => panic!("expected markup hover contents"),
        };
        assert!(
            !clean_text.contains("Stale baseline"),
            "Clean state must not append the stale footer"
        );

        let stale = hover(&chat_file, &tree, pos, content, ParseState::StaleBaseline)
            .expect("hover must resolve under StaleBaseline too");
        let stale_text = match &stale.contents {
            HoverContents::Markup(m) => &m.value,
            _ => panic!("expected markup hover contents"),
        };
        assert!(
            stale_text.contains("Stale baseline"),
            "StaleBaseline must append the footer; got:\n{stale_text}"
        );
        assert!(
            stale_text.contains("---"),
            "footer must be separated from primary content by a markdown rule"
        );
    }
}
