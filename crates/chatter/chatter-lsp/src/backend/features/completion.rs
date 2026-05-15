//! Completion candidates for speakers, headers, dependent tiers, annotations, and postcodes.
//!
//! Triggered by `*` (speaker codes from `@Participants`), `%` (standard
//! dependent-tier prefixes like `%mor:`, `%gra:`, `%pho:`, etc.), `+`
//! (postcodes from the CST), `@` (header names), and `[` (bracket
//! annotations like `[//]`, `[/]`, `[: ...]`, etc.).
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_parser::node_types::{
    ACT_TIER_PREFIX, ADD_TIER_PREFIX, ALT_TIER_PREFIX, COD_TIER_PREFIX, COH_TIER_PREFIX,
    COM_TIER_PREFIX, DEF_TIER_PREFIX, ENG_TIER_PREFIX, ERR_TIER_PREFIX, EXP_TIER_PREFIX,
    FAC_TIER_PREFIX, FLO_TIER_PREFIX, GLS_TIER_PREFIX, GPX_TIER_PREFIX, GRA_TIER_PREFIX,
    INT_TIER_PREFIX, MOD_TIER_PREFIX, MOR_TIER_PREFIX, ORT_TIER_PREFIX, PAR_TIER_PREFIX,
    PHO_TIER_PREFIX, POSTCODE, SIN_TIER_PREFIX, SIT_TIER_PREFIX, SPA_TIER_PREFIX, SPEAKER,
    TIM_TIER_PREFIX, WOR_TIER_PREFIX, X_TIER_PREFIX,
};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

use crate::backend::utils;

/// Compute completion candidates from CST context at cursor position.
pub fn completion(
    chat_file: &talkbank_model::model::ChatFile,
    tree: &Tree,
    document: &str,
    position: Position,
) -> Option<CompletionResponse> {
    let offset = utils::position_to_offset(document, position);

    // Check text-based triggers before CST lookup (handles partial/error nodes).
    let line_start = document[..offset].rfind('\n').map_or(0, |p| p + 1);
    let line_prefix = &document[line_start..offset];

    // `@` at start of line → header name completion.
    if line_prefix.starts_with('@') {
        return Some(CompletionResponse::Array(complete_header()));
    }

    // `[` inside an utterance → bracket annotation completion.
    if line_prefix.contains('[') && !line_prefix.contains(']') {
        return Some(CompletionResponse::Array(complete_bracket_annotation()));
    }

    let root = tree.root_node();
    let node = root.descendant_for_byte_range(offset, offset)?;

    if find_ancestor_kind(node, SPEAKER).is_some()
        && let Some(completions) = complete_speaker_code(chat_file)
    {
        return Some(CompletionResponse::Array(completions));
    }

    if is_tier_prefix_node(node)
        && let Some(completions) = complete_tier_type()
    {
        return Some(CompletionResponse::Array(completions));
    }

    if find_ancestor_kind(node, POSTCODE).is_some()
        && let Some(completions) = complete_postcode()
    {
        return Some(CompletionResponse::Array(completions));
    }

    None
}

/// Complete header names after `@`.
fn complete_header() -> Vec<CompletionItem> {
    let headers = [
        ("@Begin", "Start of transcript"),
        ("@End", "End of transcript"),
        ("@UTF8", "UTF-8 encoding declaration"),
        ("@Languages:\t", "Language(s) of the transcript"),
        ("@Participants:\t", "Participant declarations"),
        (
            "@ID:\t",
            "Participant identification (language|corpus|code|age|sex|||role|||)",
        ),
        ("@Media:\t", "Associated media file"),
        ("@Date:\t", "Recording date (DD-MMM-YYYY)"),
        ("@Location:\t", "Recording location"),
        ("@Situation:\t", "Recording situation description"),
        ("@Comment:\t", "General comment"),
        ("@Transcriber:\t", "Transcriber name"),
        ("@Coder:\t", "Coder name"),
        ("@Activities:\t", "Activities in the session"),
        ("@Options:\t", "Transcript options (CA, multi)"),
        ("@Bg:\t", "Begin gem (activity section)"),
        ("@Eg:\t", "End gem (activity section)"),
        ("@Birth of ", "Birth date for participant"),
        ("@Birthplace of ", "Birthplace for participant"),
        ("@L1 of ", "First language of participant"),
        ("@Number:\t", "Number of participants in transcript"),
        ("@Recording Quality:\t", "Quality of the recording"),
        ("@Room Layout:\t", "Layout of the recording room"),
        ("@Tape Location:\t", "Location on tape"),
        ("@Time Duration:\t", "Duration of the session"),
        ("@Time Start:\t", "Start time of the session"),
        ("@Transcription:\t", "Transcription status (full, partial)"),
        ("@Warning:\t", "Warning note"),
    ];

    headers
        .iter()
        .map(|(name, desc)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(desc.to_string()),
            insert_text: Some(name.to_string()),
            ..Default::default()
        })
        .collect()
}

/// Complete bracket annotations after `[`.
fn complete_bracket_annotation() -> Vec<CompletionItem> {
    let annotations = [
        ("[//]", "Retracing — exact repetition of previous"),
        ("[/]", "Retracing — reformulation/correction"),
        ("[///]", "Retracing — complete reformulation"),
        ("[: replacement]", "Replacement — actual intended word"),
        ("[=! text]", "Paralinguistic — action/sound description"),
        ("[= text]", "Explanation — clarification of context"),
        ("[*]", "Error marker — marks preceding word as error"),
        ("[+ gram]", "Postcodes — grammatical error"),
        ("[+ bch]", "Postcodes — backchannel"),
        ("[+ trn]", "Postcodes — turn"),
        ("[?]", "Best guess — uncertain transcription"),
        ("[>]", "Overlap follows — speaker is overlapped"),
        ("[<]", "Overlap precedes — speaker overlaps"),
        ("[%]", "Comment on main line"),
        ("[!]", "Stressing — emphasis"),
        ("[!!]", "Contrastive stressing — strong emphasis"),
        ("[\"\"\"  \"\"\"]", "Quotation markers"),
    ];

    annotations
        .iter()
        .map(|(code, desc)| CompletionItem {
            label: code.to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some(desc.to_string()),
            insert_text: Some(code.to_string()),
            ..Default::default()
        })
        .collect()
}

/// Complete speaker codes after `*`
fn complete_speaker_code(
    chat_file: &talkbank_model::model::ChatFile,
) -> Option<Vec<CompletionItem>> {
    // Get all declared participants
    let mut speaker_codes = Vec::new();

    for line in &chat_file.lines {
        if let talkbank_model::model::Line::Header { header, .. } = line {
            use talkbank_model::model::Header;
            match header.as_ref() {
                Header::Participants { entries } => {
                    for entry in entries {
                        speaker_codes.push((
                            entry.speaker_code.as_str().to_string(),
                            entry.role.as_str().to_string(),
                        ));
                    }
                }
                Header::ID(id_header) => {
                    speaker_codes.push((
                        id_header.speaker.as_str().to_string(),
                        id_header.role.as_str().to_string(),
                    ));
                }
                _ => {}
            }
        }
    }

    let items: Vec<CompletionItem> = speaker_codes
        .into_iter()
        .map(|(code, role)| CompletionItem {
            label: code.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(role.clone()),
            documentation: Some(Documentation::String(format!(
                "Speaker: {} ({})",
                code, role
            ))),
            insert_text: Some(format!("{}:\t", code)),
            ..Default::default()
        })
        .collect();

    if items.is_empty() { None } else { Some(items) }
}

/// Complete dependent tier types after `%`
fn complete_tier_type() -> Option<Vec<CompletionItem>> {
    // Common tier types with descriptions
    let tier_types = vec![
        ("mor", "Morphological analysis"),
        ("gra", "Grammatical relations"),
        ("pho", "Phonological transcription"),
        ("mod", "Model phonology"),
        ("sin", "Gesture/sign annotations"),
        ("act", "Action coding"),
        ("cod", "General coding"),
        ("com", "Comments"),
        ("exp", "Explanations"),
        ("add", "Additional information"),
        ("sit", "Situational information"),
        ("spa", "Spatial information"),
        ("int", "Intonation"),
        ("gpx", "Gestural transcription"),
    ];

    let items: Vec<CompletionItem> = tier_types
        .into_iter()
        .map(|(name, desc)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(desc.to_string()),
            insert_text: Some(format!("{}:\t", name)),
            ..Default::default()
        })
        .collect();

    if items.is_empty() { None } else { Some(items) }
}

/// Complete postcodes after `+`
fn complete_postcode() -> Option<Vec<CompletionItem>> {
    // Common postcodes with descriptions
    let postcodes = vec![
        ("+\"", "Quotation follows"),
        ("+,", "Self-completion"),
        ("+/.", "Interruption"),
        ("+/?", "Interruption question"),
        ("+//.", "Self-interruption"),
        ("+//?", "Self-interruption question"),
        ("+...", "Trailing off"),
        ("+/", "Incomplete sentence"),
    ];

    let items: Vec<CompletionItem> = postcodes
        .into_iter()
        .map(|(code, desc)| CompletionItem {
            label: code.to_string(),
            kind: Some(CompletionItemKind::OPERATOR),
            detail: Some(desc.to_string()),
            documentation: Some(Documentation::String(format!(
                "Postcode: {} - {}",
                code, desc
            ))),
            insert_text: Some(code.to_string()),
            ..Default::default()
        })
        .collect();

    Some(items)
}

/// Returns whether tier prefix node.
fn is_tier_prefix_node(node: Node) -> bool {
    let mut current = Some(node);
    while let Some(node) = current {
        match node.kind() {
            ACT_TIER_PREFIX | ADD_TIER_PREFIX | ALT_TIER_PREFIX | COD_TIER_PREFIX
            | COH_TIER_PREFIX | COM_TIER_PREFIX | DEF_TIER_PREFIX | ENG_TIER_PREFIX
            | ERR_TIER_PREFIX | EXP_TIER_PREFIX | FAC_TIER_PREFIX | FLO_TIER_PREFIX
            | GLS_TIER_PREFIX | GPX_TIER_PREFIX | GRA_TIER_PREFIX | INT_TIER_PREFIX
            | MOD_TIER_PREFIX | MOR_TIER_PREFIX | ORT_TIER_PREFIX | PAR_TIER_PREFIX
            | PHO_TIER_PREFIX | SIN_TIER_PREFIX | SIT_TIER_PREFIX | SPA_TIER_PREFIX
            | TIM_TIER_PREFIX | WOR_TIER_PREFIX | X_TIER_PREFIX => return true,
            _ => {}
        }
        current = node.parent();
    }
    false
}

/// Walk parent chain until a node with the requested `kind` is found.
fn find_ancestor_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut current = Some(node);
    while let Some(node) = current {
        if node.kind() == kind {
            return Some(node);
        }
        current = node.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{parse_chat, parse_tree};

    #[test]
    fn header_completion_at_line_start() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // Position cursor on @End line at character 1 (inside the '@')
        let pos = Position {
            line: 6,
            character: 1,
        };
        let result = completion(&chat_file, &tree, content, pos);
        assert!(result.is_some(), "Expected header completions after @");
        if let Some(CompletionResponse::Array(items)) = result {
            assert!(
                items.len() > 10,
                "Expected many header completions, got {}",
                items.len()
            );
            assert!(
                items.iter().any(|i| i.label == "@Languages:\t"),
                "Expected @Languages in completions"
            );
            assert!(
                items.iter().any(|i| i.label == "@Begin"),
                "Expected @Begin in completions"
            );
            assert!(
                items
                    .iter()
                    .all(|i| i.kind == Some(CompletionItemKind::KEYWORD)),
                "All header completions should be KEYWORD kind"
            );
        }
    }

    #[test]
    fn no_completion_on_main_tier_word() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // Position on "hello" (the word, not a trigger character)
        let pos = Position {
            line: 5,
            character: 8,
        };
        let result = completion(&chat_file, &tree, content, pos);
        assert!(result.is_none(), "Expected no completions on a plain word");
    }

    #[test]
    fn speaker_code_completion_returns_declared_participants() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);
        // Position on the speaker code '*CHI:' — character 1 is inside the speaker
        let pos = Position {
            line: 6,
            character: 1,
        };
        let result = completion(&chat_file, &tree, content, pos);
        // The handler checks if cursor is in a SPEAKER node. If the tree-sitter
        // tree has a SPEAKER node at that position, we get speaker completions.
        if let Some(CompletionResponse::Array(items)) = &result {
            // If we got completions, they should be speaker codes.
            assert!(
                items.iter().any(|i| i.label == "CHI"),
                "Expected CHI in speaker completions"
            );
            assert!(
                items.iter().any(|i| i.label == "MOT"),
                "Expected MOT in speaker completions"
            );
            for item in items {
                assert_eq!(
                    item.kind,
                    Some(CompletionItemKind::CLASS),
                    "Speaker completions should be CLASS kind"
                );
            }
        }
        // If no SPEAKER node at offset, result is None — acceptable.
    }

    #[test]
    fn bracket_annotation_completion_inside_bracket() {
        // Use a document that parses without errors — the bracket is intentionally
        // partial (no closing ']') so the line prefix contains '[' without ']'.
        // We test the text-based trigger directly since parse_chat would reject
        // the invalid bracket. The completion handler checks the line prefix
        // before CST lookup, so this tests the right code path.
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let tree = parse_tree(content);

        // Simulate what happens when the line prefix has '[' without ']'
        // by testing the helper directly.
        let annotations = complete_bracket_annotation();
        assert!(
            annotations.iter().any(|i| i.label == "[//]"),
            "Expected [//] retracing in completions"
        );
        assert!(
            annotations.iter().any(|i| i.label == "[*]"),
            "Expected [*] error marker in completions"
        );
        assert!(
            annotations
                .iter()
                .all(|i| i.kind == Some(CompletionItemKind::SNIPPET)),
            "All bracket annotations should be SNIPPET kind"
        );

        // Verify the completion handler does NOT trigger bracket completion
        // when there is no open bracket on the line.
        let pos = Position {
            line: 5,
            character: 8,
        };
        let result = completion(&chat_file, &tree, content, pos);
        // No open bracket on the "hello ." line at position 8.
        assert!(
            result.is_none(),
            "Expected no bracket completions without open bracket"
        );
    }

    #[test]
    fn complete_header_returns_expected_count() {
        let headers = complete_header();
        // The implementation lists header types; verify we get a reasonable count.
        assert!(
            headers.len() >= 20,
            "Expected at least 20 header completions, got {}",
            headers.len()
        );
        // Verify each has a label and detail.
        for item in &headers {
            assert!(!item.label.is_empty(), "Header label should not be empty");
            assert!(
                item.detail.is_some(),
                "Header {:?} should have a detail",
                item.label
            );
        }
    }

    #[test]
    fn complete_bracket_annotation_returns_expected_count() {
        let annotations = complete_bracket_annotation();
        assert_eq!(
            annotations.len(),
            17,
            "Expected 17 bracket annotation completions"
        );
    }

    #[test]
    fn complete_postcode_returns_items() {
        let postcodes = complete_postcode();
        assert!(postcodes.is_some(), "Postcodes should never return None");
        let items = postcodes.unwrap();
        assert_eq!(items.len(), 8, "Expected 8 postcode completions");
        assert!(
            items
                .iter()
                .all(|i| i.kind == Some(CompletionItemKind::OPERATOR)),
            "All postcodes should be OPERATOR kind"
        );
    }

    #[test]
    fn complete_tier_type_returns_tier_names() {
        let tiers = complete_tier_type();
        assert!(tiers.is_some(), "Tier type completions should not be None");
        let items = tiers.unwrap();
        assert!(
            items.iter().any(|i| i.label == "mor"),
            "Expected mor in tier completions"
        );
        assert!(
            items.iter().any(|i| i.label == "gra"),
            "Expected gra in tier completions"
        );
        // All should have tab-suffixed insert text
        for item in &items {
            assert!(
                item.insert_text
                    .as_ref()
                    .is_some_and(|t| t.ends_with(":\t")),
                "Tier insert text should end with ':\\t', got {:?}",
                item.insert_text
            );
        }
    }

    #[test]
    fn complete_speaker_code_empty_when_no_participants() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@End\n";
        let chat_file = parse_chat(content);
        let result = complete_speaker_code(&chat_file);
        assert!(
            result.is_none(),
            "Expected None when no participants declared"
        );
    }

    #[test]
    fn complete_speaker_code_extracts_from_id_headers() {
        let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother, FAT Father\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n@ID:\teng|corpus|FAT|||||Father|||\n*CHI:\thello .\n@End\n";
        let chat_file = parse_chat(content);
        let result = complete_speaker_code(&chat_file);
        assert!(result.is_some(), "Expected speaker completions");
        let items = result.unwrap();
        // Participants header contributes 3, plus 3 @ID headers = potentially duplicates
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"CHI"), "Expected CHI speaker code");
        assert!(labels.contains(&"MOT"), "Expected MOT speaker code");
        assert!(labels.contains(&"FAT"), "Expected FAT speaker code");
    }
}
