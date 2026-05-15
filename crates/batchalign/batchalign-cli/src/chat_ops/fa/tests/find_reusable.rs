//! Find/refresh-reusable-utterance-indices logic: which utterances qualify for FA reuse vs. re-alignment (`find_reusable_utterance_indices_*`, `refresh_reusable_utterances_*`).

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_find_reusable_utterance_indices_mixed_clean_stale() {
    // Utterance 0: "hello world ." with matching %wor (2 words) → reusable
    // Utterance 1: "goodbye my friend ." with stale %wor (1 word) → word count mismatch → stale
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_500\u{15} world \u{15}600_1000\u{15} .\n*CHI:\tgoodbye my friend .\n%wor:\tgoodbye \u{15}1500_2000\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(reusable.contains(&0), "utterance 0 should be reusable");
    assert!(
        !reusable.contains(&1),
        "utterance 1 should be stale (word count mismatch)"
    );
    assert_eq!(reusable.len(), 1);
}

#[test]
fn test_find_reusable_utterance_indices_all_clean() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_500\u{15} world \u{15}600_1000\u{15} .\n*CHI:\tgoodbye .\n%wor:\tgoodbye \u{15}1500_2000\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert_eq!(reusable.len(), 2);
    assert!(reusable.contains(&0));
    assert!(reusable.contains(&1));
}

#[test]
fn test_find_reusable_utterance_indices_excludes_wor_overrun_past_next_start() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world . \u{15}1000_1500\u{15}\n%wor:\thello \u{15}1100_1400\u{15} world \u{15}1400_2600\u{15} .\n*CHI:\tmhm . \u{15}2000_2400\u{15}\n%wor:\tmhm \u{15}2000_2400\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(
        !reusable.contains(&0),
        "utterance 0 should be excluded because its reused %wor span overruns the next start"
    );
    assert!(
        reusable.contains(&1),
        "utterance 1 should remain reusable because it has no following start to overrun"
    );
    assert_eq!(reusable.len(), 1);
}

#[test]
fn test_find_reusable_utterance_indices_excludes_last_word_near_zero_duration() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\talpha beta .\n%wor:\talpha \u{15}100_400\u{15} beta \u{15}400_700\u{15} .\n*CHI:\talpha beta gamma delta epsilon .\n%wor:\talpha \u{15}1000_1400\u{15} beta \u{15}1400_1800\u{15} gamma \u{15}1800_2200\u{15} delta \u{15}2200_2600\u{15} epsilon \u{15}2600_2604\u{15} .\n*CHI:\tzeta eta .\n%wor:\tzeta \u{15}3000_3300\u{15} eta \u{15}3300_3600\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(reusable.contains(&0), "utterance 0 should remain reusable");
    assert!(
        !reusable.contains(&1),
        "utterance 1 should be excluded because its final word collapses to near-zero duration"
    );
    assert!(reusable.contains(&2), "utterance 2 should remain reusable");
    assert_eq!(reusable.len(), 2);
    assert!(
        !has_reusable_wor_timing(&chat),
        "the whole file must not take the all-reusable fast path when one utterance has a collapsed final word"
    );
}

#[test]
fn test_find_reusable_utterance_indices_excludes_short_utterance_last_word_near_zero_duration() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_400\u{15} world \u{15}400_700\u{15} .\n*CHI:\talright thank you .\n%wor:\talright \u{15}1000_1500\u{15} thank \u{15}1500_1900\u{15} you \u{15}1900_1904\u{15} .\n*CHI:\tgoodbye now .\n%wor:\tgoodbye \u{15}2400_2800\u{15} now \u{15}2800_3200\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(reusable.contains(&0), "utterance 0 should remain reusable");
    assert!(
        !reusable.contains(&1),
        "utterance 1 should be excluded even though it is short because its final word collapses to near-zero duration"
    );
    assert!(reusable.contains(&2), "utterance 2 should remain reusable");
    assert_eq!(reusable.len(), 2);
    assert!(
        !has_reusable_wor_timing(&chat),
        "the whole file must not take the all-reusable fast path when a short utterance has a collapsed final word"
    );
}

#[test]
fn test_find_reusable_utterance_indices_excludes_internal_word_near_zero_duration() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_400\u{15} world \u{15}400_700\u{15} .\n*CHI:\tI have to unscrew the top .\n%wor:\tI \u{15}1000_1007\u{15} have \u{15}1007_1187\u{15} to \u{15}1187_1327\u{15} unscrew \u{15}1327_1768\u{15} the \u{15}1768_1908\u{15} top \u{15}1908_2149\u{15} .\n*CHI:\tgoodbye now .\n%wor:\tgoodbye \u{15}2400_2800\u{15} now \u{15}2800_3200\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(reusable.contains(&0), "utterance 0 should remain reusable");
    assert!(
        !reusable.contains(&1),
        "utterance 1 should be excluded because an internal word collapses to near-zero duration"
    );
    assert!(reusable.contains(&2), "utterance 2 should remain reusable");
    assert_eq!(reusable.len(), 2);
    assert!(
        !has_reusable_wor_timing(&chat),
        "the whole file must not take the all-reusable fast path when a rerun keeps an internal collapsed word"
    );
}

#[test]
fn test_find_reusable_utterance_indices_excludes_short_utterance_dominance() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_400\u{15} world \u{15}400_700\u{15} .\n*CHI:\tsorry keep going .\n%wor:\tsorry \u{15}1000_2301\u{15} keep \u{15}2301_2441\u{15} going \u{15}2441_2568\u{15} .\n*CHI:\tgoodbye now .\n%wor:\tgoodbye \u{15}3000_3400\u{15} now \u{15}3400_3800\u{15} .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(reusable.contains(&0), "utterance 0 should remain reusable");
    assert!(
        !reusable.contains(&1),
        "utterance 1 should be excluded because one word dominates a short utterance span"
    );
    assert!(reusable.contains(&2), "utterance 2 should remain reusable");
    assert_eq!(reusable.len(), 2);
    assert!(
        !has_reusable_wor_timing(&chat),
        "the whole file must not take the all-reusable fast path when a short utterance has a dominant word"
    );
}

#[test]
fn test_find_reusable_utterance_indices_no_wor() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n*CHI:\tgoodbye .\n@End\n";
    let chat = parse_chat(input);

    let reusable = find_reusable_utterance_indices(&chat);
    assert!(reusable.is_empty());
}

#[test]
fn test_refresh_reusable_utterances_selective() {
    // Utterance 0: clean %wor, utterance 1: no %wor (stale)
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_500\u{15} world \u{15}600_1000\u{15} .\n*CHI:\tgoodbye .\n@End\n";
    let mut chat = parse_chat(input);

    let reusable: std::collections::HashSet<usize> = [0].into_iter().collect();
    orchestrate::refresh_reusable_utterances(&mut chat, &reusable, true);

    let output = chat.to_chat_string();
    // Utterance 0 should have refreshed word timing
    assert!(
        output.contains("hello \u{15}100_500\u{15} world \u{15}600_1000\u{15} ."),
        "Expected refreshed main-tier timing for utt 0, got:\n{output}"
    );
    // Utterance 1 should NOT have timing (it was stale/missing %wor)
    let utt1 = get_test_utterance(&mut chat, 1);
    assert!(
        utt1.main.content.bullet.is_none(),
        "Stale utterance should not get timing from refresh"
    );
}
