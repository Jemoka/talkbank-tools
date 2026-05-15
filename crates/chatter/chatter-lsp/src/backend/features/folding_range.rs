//! Folding range provider for CHAT files.
//!
//! Creates fold regions so annotators can collapse the annotation layer
//! (`%mor:`, `%gra:`, `%pho:`, … dependent tiers) and focus on transcription.
//!
//! Each fold corresponds to one utterance *block*:
//! - `startLine` = the `*SPEAKER:` main-tier line.
//! - `endLine`   = the last `%xxx:` dependent-tier line of that utterance.
//!
//! An utterance with no dependent tiers does not produce a fold (a single-line
//! fold adds no value to the editor UI).
//!
//! Optionally, the header block (@Begin … first utterance) is folded as well,
//! using a separate `FoldingRangeKind::Region` marker.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use tower_lsp::lsp_types::*;

use crate::backend::utils::LineIndex;

/// Build the list of folding ranges for a CHAT file.
///
/// # Arguments
/// * `chat_file` - Parsed CHAT file.
/// * `document`  - Full document text (for span → line number conversion).
///
/// # Returns
/// A list of `FoldingRange` values, one per utterance block that spans more
/// than one line (i.e., has at least one dependent tier), plus an optional
/// header-block fold if a header section exists.
pub fn folding_range(
    chat_file: &talkbank_model::model::ChatFile,
    document: &str,
) -> Vec<FoldingRange> {
    let mut ranges: Vec<FoldingRange> = Vec::new();
    let index = LineIndex::new(document);

    // Track the byte offset of the first utterance for the header fold.
    let mut first_utterance_start: Option<u32> = None;

    for utterance in chat_file.utterances() {
        let main_start = utterance.main.span.start;

        if first_utterance_start.is_none() {
            first_utterance_start = Some(main_start);
        }

        // Only fold if the utterance has at least one dependent tier.
        if utterance.dependent_tiers.is_empty() {
            continue;
        }

        let main_line = index.offset_to_position(document, main_start).line;
        let block_end = utterance_block_end(utterance);
        let end_line = index.offset_to_position(document, block_end).line;

        if end_line > main_line {
            ranges.push(FoldingRange {
                start_line: main_line,
                start_character: None,
                end_line,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    // Header block fold: line 0 → line before first utterance.
    if let Some(first_start) = first_utterance_start {
        let first_utt_line = index.offset_to_position(document, first_start).line;
        if first_utt_line > 1 {
            ranges.push(FoldingRange {
                start_line: 0,
                start_character: None,
                end_line: first_utt_line.saturating_sub(1),
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    // Gem block folds: @Bg: Activity → @Eg: Activity.
    let mut gem_stack: Vec<(u32, &str)> = Vec::new(); // (line, label)
    for (i, line) in document.lines().enumerate() {
        let line_num = i as u32;
        if let Some(label) = line.strip_prefix("@Bg:\t") {
            gem_stack.push((line_num, label.trim()));
        } else if line.starts_with("@Eg:")
            && let Some((start_line, _label)) = gem_stack.pop()
            && line_num > start_line
        {
            ranges.push(FoldingRange {
                start_line,
                start_character: None,
                end_line: line_num,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    ranges
}

/// Returns the byte offset of the end of an utterance block (main tier +
/// all dependent tiers). Falls back to the main tier end when no dependent
/// tiers are present.
fn utterance_block_end(utterance: &talkbank_model::model::Utterance) -> u32 {
    utterance
        .dependent_tiers
        .iter()
        .map(|t| t.span().end)
        .max()
        .unwrap_or(utterance.main.span.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::parse_chat;

    #[test]
    fn fold_utterance_with_dependent_tiers() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n%mor:\tn|hello n|world .\n%gra:\t1|2|SUBJ 2|0|ROOT 3|2|OBJ\n@End\n";
        let chat_file = parse_chat(input);
        let ranges = folding_range(&chat_file, input);

        // Should have at least one utterance fold and a header fold
        assert!(ranges.len() >= 2);

        // Find the utterance fold (starts at the *CHI: line)
        let utt_fold = ranges.iter().find(|r| r.start_line == 5).unwrap();
        assert!(utt_fold.end_line > utt_fold.start_line);
    }

    #[test]
    fn no_fold_for_single_line_utterance() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(input);
        let ranges = folding_range(&chat_file, input);

        // No utterance fold — only header fold (if any)
        assert!(
            ranges
                .iter()
                .all(|r| r.start_line == 0 || r.start_line != 5)
        );
    }

    #[test]
    fn header_block_fold() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(input);
        let ranges = folding_range(&chat_file, input);

        // Header fold starts at line 0
        let header_fold = ranges.iter().find(|r| r.start_line == 0);
        assert!(header_fold.is_some());
        assert!(header_fold.unwrap().end_line >= 3);
    }

    #[test]
    fn gem_block_fold() {
        let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n@Bg:\tActivity\n*CHI:\thello .\n@Eg:\tActivity\n@End\n";
        let chat_file = parse_chat(input);
        let ranges = folding_range(&chat_file, input);

        // Should have a gem fold from @Bg to @Eg
        let gem_fold = ranges.iter().find(|r| {
            let start_line = input.lines().nth(r.start_line as usize).unwrap_or("");
            start_line.starts_with("@Bg:")
        });
        assert!(gem_fold.is_some());
    }
}
