//! Utterance grouping (`group_utterances_*`), `%wor` extraction policy, and reusable-wor detection (`has_reusable_wor_timing_*`, `refresh_existing_alignment_*`).

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_group_utterances_single_group() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_two_timed_utterances.cha");
    let chat = parse_chat(input);
    let groups = group_utterances(&chat, 20000, None);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].words.len(), 5); // hello world I want cookie
    assert_eq!(groups[0].audio_start_ms(), 0);
    assert_eq!(groups[0].audio_end_ms(), 10000);
}

#[test]
fn test_wor_policy_fillers_match_between_fa_extraction_and_wor_generation() {
    let main = "&-um there .";

    assert_eq!(collect_proof_fa_words(main), vec!["um", "there"]);
    assert_eq!(generate_proof_wor_words(main), vec!["um", "there"]);
}

#[test]
fn test_wor_policy_replacements_use_original_surface() {
    let main = "what's is dis [: this] ?";

    assert_eq!(collect_proof_fa_words(main), vec!["what's", "is", "dis"]);
    assert_eq!(generate_proof_wor_words(main), vec!["what's", "is", "dis"]);
}

#[test]
fn test_wor_policy_standalone_spoken_tokens_match_between_fa_extraction_and_wor_generation() {
    for (main, expected) in [
        // Fillers (&-) ARE included
        ("&-um play .", &["um", "play"][..]),
        // Fragments (&+) are excluded — BA2 TokenType.ANNOT
        ("&+ss play .", &["play"][..]),
        // Nonwords (&~) are excluded — BA2 TokenType.ANNOT
        ("&~um play .", &["play"][..]),
    ] {
        let expected = words(expected);
        assert_eq!(collect_proof_fa_words(main), expected);
        assert_eq!(generate_proof_wor_words(main), expected);
    }
}

#[test]
fn test_wor_policy_retraced_spoken_tokens_match_between_fa_extraction_and_wor_generation() {
    for (main, expected) in [
        // Fragments excluded even inside retrace
        ("<&+ss> [/] play .", &["play"][..]),
        // Nonwords excluded even inside retrace
        ("<&~um> [/] play .", &["play"][..]),
        // Fillers still included inside retrace
        ("<&-um> [/] play .", &["um", "play"][..]),
        // Untranscribed excluded in all contexts
        ("<xxx> [/] play .", &["play"][..]),
        ("<yyy> [/] play .", &["play"][..]),
        ("<www> [/] play .", &["play"][..]),
    ] {
        let expected = words(expected);
        assert_eq!(collect_proof_fa_words(main), expected);
        assert_eq!(generate_proof_wor_words(main), expected);
    }
}

#[test]
fn test_group_utterances_backwards_bullets() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_backwards_bullets.cha");
    let chat = parse_chat(input);
    let groups = group_utterances(&chat, 20000, None);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].words.len(), 1);
    assert_eq!(groups[1].words.len(), 1);
}

#[test]
fn test_group_utterances_splits_on_time() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_split_on_time.cha");
    let chat = parse_chat(input);
    let groups = group_utterances(&chat, 20000, None);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].words.len(), 1);
    assert_eq!(groups[1].words.len(), 1);
}

#[test]
fn test_wor_policy_untranscribed_tokens_excluded_from_fa_and_wor() {
    for (main, expected) in [
        // Pure untranscribed utterances produce no FA words and empty %wor
        ("xxx .", &[][..]),
        ("yyy .", &[][..]),
        ("www .", &[][..]),
        // Mixed: real words stay; untranscribed tokens are dropped
        ("xxx play .", &["play"][..]),
        ("yyy play .", &["play"][..]),
        ("www play .", &["play"][..]),
        ("hello xxx world .", &["hello", "world"][..]),
        // Fragments excluded (BA2 TokenType.ANNOT)
        ("&+ss play .", &["play"][..]),
        // Nonwords excluded (BA2 TokenType.ANNOT)
        ("&~um play .", &["play"][..]),
        // Fillers included (BA2 TokenType.FP)
        ("&-um play .", &["um", "play"][..]),
    ] {
        let expected = words(expected);
        assert_eq!(
            collect_proof_fa_words(main),
            expected,
            "FA extraction for: {main}"
        );
        assert_eq!(
            generate_proof_wor_words(main),
            expected,
            "%wor generation for: {main}"
        );
    }
}

#[test]
fn test_group_utterances_splits_on_whisper_token_limit() {
    // Two utterances each with 50 five-character words = 250 chars per utterance.
    // Combined = 500 chars > WHISPER_FA_MAX_LABEL_TOKENS (448).
    // Both fit in a 60-second window, so without the char limit they'd be one group.
    let fifty_words = vec!["abcde"; 50].join(" ");
    let chat_text = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|test|CHI|||||Child|||\n*CHI:\t{fifty_words} .\x15100_5000\x15\n*CHI:\t{fifty_words} .\x155000_10000\x15\n@End\n"
    );
    let chat = parse_chat(&chat_text);
    let groups = group_utterances(&chat, 60_000, Some(10_000));
    assert_eq!(
        groups.len(),
        2,
        "expected 2 groups (50+50 words × 5 chars = 500 > 448 token limit), got {}",
        groups.len()
    );
    // Every group must stay within the Whisper token limit.
    for (i, group) in groups.iter().enumerate() {
        let chars: usize = group.words.iter().map(|w| w.text.len()).sum();
        assert!(
            chars <= WHISPER_FA_MAX_LABEL_TOKENS,
            "group {i} has {chars} chars, exceeds {WHISPER_FA_MAX_LABEL_TOKENS} token limit"
        );
    }
}

#[test]
fn test_group_utterances_skips_untimed() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_mixed_timed_untimed.cha");
    let chat = parse_chat(input);
    let groups = group_utterances(&chat, 20000, None);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].words.len(), 1); // only "world"
}

#[test]
fn test_has_reusable_wor_timing_true_for_complete_wor_roundtrip() {
    let chat = parse_chat(&wor_timed_chat());
    assert!(has_reusable_wor_timing(&chat));
}

#[test]
fn test_has_reusable_wor_timing_false_for_partial_wor_timing() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world .\n%wor:\thello \u{15}100_500\u{15} world .\n@End\n".to_string();
    let chat = parse_chat(&input);
    assert!(!has_reusable_wor_timing(&chat));
}

#[test]
fn test_has_reusable_wor_timing_false_when_wor_overruns_next_start() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello world . \u{15}1000_1500\u{15}\n%wor:\thello \u{15}1100_1400\u{15} world \u{15}1400_2600\u{15} .\n*CHI:\tmhm . \u{15}2000_2400\u{15}\n%wor:\tmhm \u{15}2000_2400\u{15} .\n@End\n";
    let chat = parse_chat(input);
    assert!(
        !has_reusable_wor_timing(&chat),
        "a %wor span that runs past the next utterance start must not qualify for whole-file reuse"
    );
}

#[test]
fn test_has_reusable_wor_timing_false_when_one_word_dominates_utterance_span() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\talpha beta gamma delta .\n%wor:\talpha \u{15}100_200\u{15} beta \u{15}200_5800\u{15} gamma \u{15}5800_5900\u{15} delta \u{15}5900_6000\u{15} .\n@End\n";
    let chat = parse_chat(input);
    let utt = get_utterance(&chat, 0);
    assert!(
        !has_reusable_wor_timing_for_utterance(utt),
        "a %wor timing distribution with one word consuming most of the utterance span must not qualify for cheap reuse"
    );
}

#[test]
fn test_has_reusable_wor_timing_false_when_one_word_dominates_short_utterance_span() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\tsorry keep going .\n%wor:\tsorry \u{15}100_1401\u{15} keep \u{15}1401_1541\u{15} going \u{15}1541_1668\u{15} .\n@End\n";
    let chat = parse_chat(input);
    let utt = get_utterance(&chat, 0);
    assert!(
        !has_reusable_wor_timing_for_utterance(utt),
        "a short utterance whose first word consumes most of the span must not qualify for cheap reuse"
    );
}

#[test]
fn test_has_reusable_wor_timing_false_when_last_word_collapses_to_near_zero_duration() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\talpha beta gamma delta epsilon .\n%wor:\talpha \u{15}100_500\u{15} beta \u{15}500_900\u{15} gamma \u{15}900_1300\u{15} delta \u{15}1300_1700\u{15} epsilon \u{15}1700_1704\u{15} .\n@End\n";
    let chat = parse_chat(input);
    let utt = get_utterance(&chat, 0);
    assert!(
        !has_reusable_wor_timing_for_utterance(utt),
        "a %wor timing distribution whose final word collapses to near-zero duration must not qualify for cheap reuse"
    );
}

#[test]
fn test_refresh_existing_alignment_rehydrates_main_tier_from_wor() {
    let mut chat = parse_chat(&wor_timed_chat());
    refresh_existing_alignment(&mut chat, true);

    let output = chat.to_chat_string();
    assert!(
        output.contains("hello \u{15}100_500\u{15} world \u{15}600_1000\u{15} ."),
        "Expected refreshed main-tier word timing, got:\n{output}"
    );
    assert!(
        output.contains("%wor:\thello \u{15}100_500\u{15} world \u{15}600_1000\u{15} ."),
        "Expected refreshed %wor tier, got:\n{output}"
    );
}

#[test]
fn test_group_utterances_includes_untimed_with_interpolation() {
    let input =
        include_str!("../../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");
    let chat = parse_chat(input);
    let groups = group_utterances(&chat, 20000, Some(50000));

    // All 6 utterances should be included (none skipped)
    let total_utts: usize = groups.iter().map(|g| g.utterance_indices.len()).sum();
    assert_eq!(total_utts, 6);
}
