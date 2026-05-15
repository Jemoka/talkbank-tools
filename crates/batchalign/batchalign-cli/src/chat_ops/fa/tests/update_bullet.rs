//! Bullet update / preservation rules: `update_utterance_bullet_*`, `apply_fa_results_preserves_pretimed_bullet`.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_update_utterance_bullet_preserves_start_with_leading_fillers() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_pretimed_with_fillers.cha");
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    // Verify pre-existing bullet
    let original_bullet = utt.main.content.bullet.clone().unwrap();
    assert_eq!(original_bullet.timing.start_ms, 37397);
    assert_eq!(original_bullet.timing.end_ms, 42983);

    // Simulate FA: only "I", "went", "home" get timed (filler &-uh does not)
    let timings = vec![
        Some(WordTiming::new(42221, 42582)),
        Some(WordTiming::new(42582, 42782)),
        Some(WordTiming::new(42782, 42983)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    postprocess_utterance_timings(utt, FaTimingMode::WithPauses);
    update_utterance_bullet(utt);

    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(
        bullet.timing.start_ms, 37397,
        "Bullet start must be preserved from original (covers leading filler), got {}",
        bullet.timing.start_ms,
    );
    assert_eq!(
        bullet.timing.end_ms, 42983,
        "Bullet end must be preserved from original, got {}",
        bullet.timing.end_ms,
    );
}

#[test]
fn test_update_utterance_bullet_preserves_end_with_trailing_gesture() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_pretimed_trailing_gesture.cha");
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    // Verify pre-existing bullet
    let original_bullet = utt.main.content.bullet.clone().unwrap();
    assert_eq!(original_bullet.timing.start_ms, 50556);
    assert_eq!(original_bullet.timing.end_ms, 56221);

    // Simulate FA: "and", "it", "screwed", "up" get timed; &=laughs does not
    let timings = vec![
        Some(WordTiming::new(50616, 52596)),
        Some(WordTiming::new(52596, 54637)),
        Some(WordTiming::new(54637, 55718)),
        Some(WordTiming::new(55718, 55898)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    postprocess_utterance_timings(utt, FaTimingMode::WithPauses);
    update_utterance_bullet(utt);

    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(
        bullet.timing.start_ms, 50556,
        "Bullet start must be preserved from original, got {}",
        bullet.timing.start_ms,
    );
    assert_eq!(
        bullet.timing.end_ms, 56221,
        "Bullet end must be preserved from original (covers trailing gesture), got {}",
        bullet.timing.end_ms,
    );
}

#[test]
fn test_update_utterance_bullet_sets_new_bullet_when_none_existed() {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n@Media:\ttest, audio\n*CHI:\thello world .\n@End\n";
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    // No pre-existing bullet
    assert!(utt.main.content.bullet.is_none());

    let timings = vec![
        Some(WordTiming::new(100, 500)),
        Some(WordTiming::new(600, 1000)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    update_utterance_bullet(utt);

    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(bullet.timing.start_ms, 100);
    assert_eq!(bullet.timing.end_ms, 1000);
}

#[test]
fn test_apply_fa_results_preserves_pretimed_bullet() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_pretimed_with_fillers.cha");
    let mut chat = parse_chat(input);

    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(37397, 42983),
        words: vec![
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(0),
                text: "I".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(1),
                text: "went".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(2),
                text: "home".into(),
            },
        ],
        utterance_indices: vec![UtteranceIdx(0)],
    }];

    let responses = vec![vec![
        Some(WordTiming::new(42221, 42582)),
        Some(WordTiming::new(42582, 42782)),
        Some(WordTiming::new(42782, 42983)),
    ]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        true,
    );

    let utt = get_test_utterance(&mut chat, 0);
    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(
        bullet.timing.start_ms, 37397,
        "Pipeline must preserve original bullet start (covers leading filler), got {}",
        bullet.timing.start_ms,
    );
    assert_eq!(
        bullet.timing.end_ms, 42983,
        "Pipeline must preserve original bullet end, got {}",
        bullet.timing.end_ms,
    );
}

#[test]
fn test_update_utterance_bullet_expands_when_words_exceed_original() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_pretimed_with_fillers.cha");
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    // Original bullet: 37397_42983
    // Simulate FA returning words that start before and end after the bullet
    let timings = vec![
        Some(WordTiming::new(37000, 38000)),
        Some(WordTiming::new(38000, 43500)),
        Some(WordTiming::new(43500, 44000)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    // Skip postprocess (it would clamp to utterance boundary) — test update only
    update_utterance_bullet(utt);

    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(
        bullet.timing.start_ms, 37000,
        "Bullet should expand to earlier word start, got {}",
        bullet.timing.start_ms,
    );
    assert_eq!(
        bullet.timing.end_ms, 44000,
        "Bullet should expand to later word end, got {}",
        bullet.timing.end_ms,
    );
}

#[test]
fn test_update_utterance_bullet_discards_large_stale_start_on_rerun_without_leading_filler() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tPAR Participant\n\
@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\ttest, audio\n\
*PAR:\thow did this happen ? \u{0015}2000_9970\u{0015}\n\
%wor:\thow did this happen ?\n\
@End\n\
";
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    let timings = vec![
        Some(WordTiming::new(9443, 9643)),
        Some(WordTiming::new(9643, 9783)),
        Some(WordTiming::new(9783, 9970)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    update_utterance_bullet(utt);

    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(
        bullet.timing.start_ms, 9443,
        "Large stale start with no untimed leading content must be replaced by first word start, got {}",
        bullet.timing.start_ms,
    );
    assert_eq!(
        bullet.timing.end_ms, 9970,
        "Bullet end should still reflect the last word end, got {}",
        bullet.timing.end_ms,
    );
}

#[test]
fn test_update_utterance_bullet_discards_large_stale_start_when_leading_filler_is_timed() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tPAR Participant\n\
@ID:\teng|test|PAR|||||Participant|||\n\
@Media:\ttest, audio\n\
*PAR:\t&-uh you want me to talk as I'm going ? \u{0015}3480_10590\u{0015}\n\
%wor:\tuh you want me to talk as I'm going ?\n\
@End\n\
";
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    let timings = vec![
        Some(WordTiming::new(8473, 8693)),
        Some(WordTiming::new(8693, 8813)),
        Some(WordTiming::new(8813, 8954)),
        Some(WordTiming::new(8954, 9074)),
        Some(WordTiming::new(9074, 9815)),
        Some(WordTiming::new(9815, 10136)),
        Some(WordTiming::new(10136, 10216)),
        Some(WordTiming::new(10216, 10437)),
        Some(WordTiming::new(10437, 10590)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    update_utterance_bullet(utt);

    let bullet = utt.main.content.bullet.as_ref().unwrap();
    assert_eq!(
        bullet.timing.start_ms, 8473,
        "Large stale start must snap to the timed leading filler, got {}",
        bullet.timing.start_ms,
    );
    assert_eq!(
        bullet.timing.end_ms, 10590,
        "Bullet end should still reflect the final word end, got {}",
        bullet.timing.end_ms,
    );
}
