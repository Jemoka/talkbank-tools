//! UTR token injection edge cases (`utr_*`, `test_utr_*`) and monotonicity-clamp zero-duration prevention.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn utr_serialize_reparse_no_internal_bullets() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_untimed_for_utr.cha");
    let mut chat = parse_chat(input);

    // Inject UTR timing (synthetic ASR tokens matching the words)
    let tokens = make_utr_tokens(&[
        ("hello", 1000, 1500),
        ("world", 1600, 2000),
        ("goodbye", 3000, 3500),
        ("world", 3600, 4000),
        ("more", 5000, 5400),
        ("words", 5500, 5800),
        ("here", 5900, 6200),
    ]);
    let result = utr::inject_utr_timing(&mut chat, &tokens);
    assert!(result.injected > 0, "UTR should inject timing");

    // At this point, ChatFile has TierContent.bullet set, but NO InternalBullet items
    assert_eq!(
        count_internal_bullets(&chat),
        0,
        "After UTR injection: should have zero InternalBullet items in AST"
    );

    // Serialize to CHAT text (this is what the old pipeline did)
    let serialized = talkbank_transform::serialize::to_chat_string(&chat);

    // Re-parse (this is what FA did — the bug)
    let reparsed = parse_chat(&serialized);

    // With the CA terminator resolution fix, the parser correctly promotes
    // trailing bullets to terminal TierContent.bullet — no InternalBullets.
    let internal_count = count_internal_bullets(&reparsed);
    assert_eq!(
        internal_count, 0,
        "After serialize→re-parse: parser should promote all bullets to terminal \
         (CA terminator resolution). Found {internal_count} InternalBullet items."
    );
}

#[test]
fn apply_fa_produces_no_double_bullets_after_utr() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_untimed_for_utr.cha");
    let mut chat = parse_chat(input);
    let tokens = make_utr_tokens(&[
        ("hello", 1000, 1500),
        ("world", 1600, 2000),
        ("goodbye", 3000, 3500),
        ("world", 3600, 4000),
        ("more", 5000, 5400),
        ("words", 5500, 5800),
        ("here", 5900, 6200),
    ]);
    utr::inject_utr_timing(&mut chat, &tokens);

    // Group and create synthetic FA timings
    let groups = group_utterances(&chat, 30_000, Some(10_000));
    let responses: Vec<Vec<Option<WordTiming>>> = groups
        .iter()
        .map(|g| {
            let word_count: usize = g
                .utterance_indices
                .iter()
                .map(|&idx| {
                    let mut count = 0;
                    for (i, line) in chat.lines.iter().enumerate() {
                        if let Line::Utterance(u) = line
                            && UtteranceIdx(i) == idx
                        {
                            count = count_alignable_main_words(u);
                        }
                    }
                    count
                })
                .sum();
            (0..word_count)
                .map(|i| {
                    Some(WordTiming {
                        start_ms: (i as u64) * 500 + 1000,
                        end_ms: (i as u64) * 500 + 1400,
                    })
                })
                .collect()
        })
        .collect();

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::Continuous,
        true,
    );

    let output = talkbank_transform::serialize::to_chat_string(&chat);
    let double_bullets = count_double_bullet_lines(&output);
    assert_eq!(
        double_bullets, 0,
        "After apply_fa_results: should have zero double-bullet lines, got {double_bullets}.\n\
         Output:\n{output}"
    );
}

#[test]
fn test_utr_zero_duration_asr_token_does_not_create_zero_duration_bullet() {
    // Single-word backchannel — the canonical OCSC failure pattern.
    let mut chat = parse_chat(&proof_chat("mhm ."));

    // Simulate Whisper returning start==end for "mhm" (one 20ms frame).
    let tokens = make_utr_tokens(&[("mhm", 646025, 646025)]);

    let result = utr::inject_utr_timing(&mut chat, &tokens);

    // The utterance must NOT receive a zero-duration bullet.
    // Either it stays untimed (unmatched) or gets a non-zero-duration bullet.
    let bullet = get_utterance_bullet(&chat, 0);
    match bullet {
        Some((start, end)) => {
            assert!(
                start < end,
                "UTR injected a zero-duration bullet •{start}_{end}• for \"mhm\" — \
                 this is the OCSC bug: Whisper zero-duration ASR token propagated to \
                 utterance bullet, which perpetuates through all FA re-runs"
            );
        }
        None => {
            // Correct outcome: utterance is untimed; FA will assign a valid bullet.
            assert_eq!(
                result.unmatched, 1,
                "utterance should be counted as unmatched when only \
                 zero-duration tokens are available"
            );
        }
    }
}

#[test]
fn test_utr_non_overlap_utterances_get_strictly_increasing_start_times() {
    use utr::UtrStrategy;
    // Two adjacent non-overlap utterances.  Both ASR tokens share start=1000ms
    // (the Whisper 20ms DTW artifact — both backchannels fall in the same frame).
    let chat_text = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\tmhm .\n\
*CHI:\tyeah .\n\
@End\n\
";
    let mut chat = parse_chat(chat_text);
    let tokens = make_utr_tokens(&[
        ("mhm", 1000, 1500),
        ("yeah", 1000, 2000), // same start_ms as "mhm" — Whisper DTW collision
    ]);

    let _result = utr::TwoPassOverlapUtr::new().inject(&mut chat, &tokens);

    let b0 = get_utterance_bullet(&chat, 0);
    let b1 = get_utterance_bullet(&chat, 1);

    // Both utterances should be timed.
    let (start0, _end0) = b0.expect("utterance 0 (mhm) should have a bullet");
    let (start1, _end1) = b1.expect("utterance 1 (yeah) should have a bullet");

    // The core invariant: adjacent non-overlap utterances must have strictly
    // increasing start times so that monotonicity end-clamping can never
    // produce a zero-duration bullet.
    assert!(
        start1 > start0,
        "adjacent non-overlap utterances must have strictly increasing start_ms: \
         utt0.start={start0} utt1.start={start1} — identical start times from \
         Whisper DTW collision will cause enforce_monotonicity to produce •T_T•"
    );
}

#[test]
fn test_monotonicity_clamp_does_not_create_zero_duration_bullet() {
    // Two utterances with identical start times — the UTR overlap scenario.
    // Utterance 0: •1000_1500• (start=1000, end=1500)
    // Utterance 1: •1000_2000• (start=1000 == utt0.start → monotonicity
    //              pass 1 leaves it because 1000 >= last_start_ms=1000,
    //              then pass 2 clamps utt0.end to 1000 → 1000_1000).
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\thello . \u{0015}1000_1500\u{0015}\n\
*CHI:\tworld . \u{0015}1000_2000\u{0015}\n\
@End\n\
";
    let mut chat = parse_chat(input);
    enforce_monotonicity(&mut chat);

    // After monotonicity enforcement, no bullet may have start_ms >= end_ms.
    for (i, line) in chat.lines.iter().enumerate() {
        let talkbank_model::model::Line::Utterance(utt) = line else {
            continue;
        };
        if let Some(bullet) = &utt.main.content.bullet {
            assert!(
                bullet.timing.start_ms < bullet.timing.end_ms,
                "utterance at line {i} has zero-or-negative-duration bullet \
                 •{}_{} after monotonicity enforcement — this is the UTR overlap \
                 bug: identical start times cause end-clamping to produce •T_T•",
                bullet.timing.start_ms,
                bullet.timing.end_ms
            );
        }
    }
}
