//! Document symbol provider for CHAT files.
//!
//! Returns a two-level symbol tree for VS Code's Outline panel, breadcrumb bar,
//! and `Cmd+Shift+O` symbol picker:
//!
//! ```text
//! Chat Transcript         (Module — whole file)
//!   Headers               (Namespace — @Begin → first utterance)
//!   Utterances            (Namespace — first utterance → @End)
//!     *CHI: hello         (String — line N)
//!     *MOT: hi            (String — line M)
//! ```
//!
//! Spans are computed from the byte-offset `Span` stored in each utterance's
//! `main` tier, converted to LSP `Position` (line/character) via the existing
//! `offset_to_position` utility.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use tower_lsp::lsp_types::*;

use crate::backend::utils::LineIndex;

/// Build the document symbol tree for a CHAT file.
///
/// # Arguments
/// * `chat_file` - Parsed CHAT file (may be unvalidated).
/// * `document`  - Full document text (needed for span → line conversion).
///
/// # Returns
/// A flat list whose first element is the top-level "Chat Transcript" module
/// containing nested "Headers" and "Utterances" namespaces, each of which
/// contains one child symbol per utterance.
pub fn document_symbol(
    chat_file: &talkbank_model::model::ChatFile,
    document: &str,
) -> Option<DocumentSymbolResponse> {
    let doc_len = document.len();
    let index = LineIndex::new(document);

    // Compute the full-file range (start of file → end of file).
    let file_start = Position {
        line: 0,
        character: 0,
    };
    let file_end = index.offset_to_position(document, doc_len as u32);
    let file_range = Range {
        start: file_start,
        end: file_end,
    };

    // Build one child symbol per utterance.
    let mut utterance_children: Vec<DocumentSymbol> = Vec::new();

    // Track the byte offset of the first utterance to delimit the "Headers" section.
    let mut first_utterance_offset: Option<u32> = None;

    for utterance in chat_file.utterances() {
        let main_span = utterance.main.span;
        if first_utterance_offset.is_none() {
            first_utterance_offset = Some(main_span.start);
        }

        // Compute the full utterance block extent (main + all dependent tiers).
        let block_start = main_span.start;
        let block_end = utterance_block_end(utterance);

        let start_pos = index.offset_to_position(document, block_start);
        let end_pos = index.offset_to_position(document, block_end);

        // Label: "*SPEAKER: …" truncated at 60 chars for readability.
        let speaker = utterance.main.speaker.as_str();
        // Collect the first line of text from the main tier for the label.
        let label_text = first_line_at_offset(document, block_start as usize, 60);
        let label = if label_text.is_empty() {
            format!("*{speaker}:")
        } else {
            label_text
        };

        #[allow(deprecated)] // `deprecated` field required by LSP spec
        let sym = DocumentSymbol {
            name: label,
            detail: None,
            kind: SymbolKind::STRING,
            tags: None,
            deprecated: None,
            range: Range {
                start: start_pos,
                end: end_pos,
            },
            selection_range: Range {
                start: start_pos,
                end: start_pos,
            },
            children: None,
        };
        utterance_children.push(sym);
    }

    // -----------------------------------------------------------------------
    // Build gem block symbols from @Bg/@Eg pairs.
    // -----------------------------------------------------------------------
    let mut gem_stack: Vec<(u32, String)> = Vec::new(); // (start_offset, label)
    let mut gem_symbols: Vec<DocumentSymbol> = Vec::new();
    let mut byte_pos: u32 = 0;
    for line in document.lines() {
        let line_len = line.len() as u32 + 1; // +1 for newline
        if let Some(label) = line.strip_prefix("@Bg:\t") {
            gem_stack.push((byte_pos, label.trim().to_string()));
        } else if line.starts_with("@Eg:")
            && let Some((start_off, label)) = gem_stack.pop()
        {
            let end_off = byte_pos + line_len - 1;
            let start_pos = index.offset_to_position(document, start_off);
            let end_pos = index.offset_to_position(document, end_off);
            #[allow(deprecated)]
            gem_symbols.push(DocumentSymbol {
                name: format!("Gem: {label}"),
                detail: None,
                kind: SymbolKind::EVENT,
                tags: None,
                deprecated: None,
                range: Range {
                    start: start_pos,
                    end: end_pos,
                },
                selection_range: Range {
                    start: start_pos,
                    end: start_pos,
                },
                children: None,
            });
        }
        byte_pos += line_len;
    }

    // Merge gem symbols into utterance children so they appear in the outline.
    utterance_children.extend(gem_symbols);
    // Sort all children by start line for consistent ordering.
    utterance_children.sort_by_key(|s| s.range.start.line);

    // -----------------------------------------------------------------------
    // Build the "Headers" namespace symbol.
    // -----------------------------------------------------------------------
    let headers_end_pos = first_utterance_offset
        .map(|off| index.offset_to_position(document, off.saturating_sub(1)))
        .unwrap_or(file_end);

    #[allow(deprecated)]
    let headers_sym = DocumentSymbol {
        name: "Headers".to_string(),
        detail: None,
        kind: SymbolKind::NAMESPACE,
        tags: None,
        deprecated: None,
        range: Range {
            start: file_start,
            end: headers_end_pos,
        },
        selection_range: Range {
            start: file_start,
            end: file_start,
        },
        children: None,
    };

    // -----------------------------------------------------------------------
    // Build the "Utterances" namespace symbol wrapping all utterance children.
    // -----------------------------------------------------------------------
    let utterances_start = first_utterance_offset
        .map(|off| index.offset_to_position(document, off))
        .unwrap_or(file_end);

    #[allow(deprecated)]
    let utterances_sym = DocumentSymbol {
        name: "Utterances".to_string(),
        detail: Some(format!("{} utterances", utterance_children.len())),
        kind: SymbolKind::NAMESPACE,
        tags: None,
        deprecated: None,
        range: Range {
            start: utterances_start,
            end: file_end,
        },
        selection_range: Range {
            start: utterances_start,
            end: utterances_start,
        },
        children: Some(utterance_children),
    };

    // -----------------------------------------------------------------------
    // Root "Chat Transcript" module symbol.
    // -----------------------------------------------------------------------
    #[allow(deprecated)]
    let root_sym = DocumentSymbol {
        name: "Chat Transcript".to_string(),
        detail: None,
        kind: SymbolKind::MODULE,
        tags: None,
        deprecated: None,
        range: file_range,
        selection_range: Range {
            start: file_start,
            end: file_start,
        },
        children: Some(vec![headers_sym, utterances_sym]),
    };

    Some(DocumentSymbolResponse::Nested(vec![root_sym]))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the byte offset of the end of an utterance block (main tier +
/// all dependent tiers). Falls back to the main tier end if no dependent
/// tiers are present.
fn utterance_block_end(utterance: &talkbank_model::model::Utterance) -> u32 {
    utterance
        .dependent_tiers
        .iter()
        .map(|t| t.span().end)
        .max()
        .unwrap_or(utterance.main.span.end)
}

/// Returns the first `max_chars` characters of the line that starts at
/// `byte_offset` in `document`, stripping any leading tab characters.
fn first_line_at_offset(document: &str, byte_offset: usize, max_chars: usize) -> String {
    let text = &document[byte_offset.min(document.len())..];
    let line = text.lines().next().unwrap_or("");
    let trimmed = line.trim_start_matches('\t');
    trimmed.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_chat;

    #[test]
    fn returns_nested_symbol_tree() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(input);
        let response = document_symbol(&chat_file, input);
        assert!(response.is_some());

        if let Some(DocumentSymbolResponse::Nested(symbols)) = response {
            assert_eq!(symbols.len(), 1); // Root "Chat Transcript"
            assert_eq!(symbols[0].name, "Chat Transcript");
            assert_eq!(symbols[0].kind, SymbolKind::MODULE);

            let children = symbols[0].children.as_ref().unwrap();
            assert_eq!(children.len(), 2); // Headers + Utterances
            assert_eq!(children[0].name, "Headers");
            assert_eq!(children[1].name, "Utterances");
        } else {
            panic!("Expected nested response");
        }
    }

    #[test]
    fn utterances_contain_speaker_labels() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n*MOT:\thi there .\n@End\n";
        let chat_file = parse_chat(input);
        let response = document_symbol(&chat_file, input);

        if let Some(DocumentSymbolResponse::Nested(symbols)) = response {
            let utterances = &symbols[0].children.as_ref().unwrap()[1];
            let utt_children = utterances.children.as_ref().unwrap();
            assert_eq!(utt_children.len(), 2);
            assert!(utt_children[0].name.contains("CHI"));
            assert!(utt_children[1].name.contains("MOT"));
        } else {
            panic!("Expected nested response");
        }
    }

    #[test]
    fn gem_blocks_appear_as_event_symbols() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n@Bg:\tPlay\n*CHI:\thello .\n@Eg:\tPlay\n@End\n";
        let chat_file = parse_chat(input);
        let response = document_symbol(&chat_file, input);

        if let Some(DocumentSymbolResponse::Nested(symbols)) = response {
            let utterances = &symbols[0].children.as_ref().unwrap()[1];
            let children = utterances.children.as_ref().unwrap();
            // Should contain the utterance AND the gem
            assert!(children.iter().any(|s| s.name.contains("Gem: Play")));
            assert!(children.iter().any(|s| s.kind == SymbolKind::EVENT));
        } else {
            panic!("Expected nested response");
        }
    }

    #[test]
    fn first_line_at_offset_truncates() {
        let text = "\tthis is a very long line that goes on and on";
        let result = first_line_at_offset(text, 0, 10);
        assert_eq!(result.len(), 10);
        assert!(!result.starts_with('\t')); // Tab stripped
    }
}
