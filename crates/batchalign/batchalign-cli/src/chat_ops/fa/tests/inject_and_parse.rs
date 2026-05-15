//! Per-utterance timing injection, FA result parsing (token-level + indexed), boundary estimation, fa-cache-key generation, snapshot probe, and bookkeeping miscellany.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_inject_timings_simple() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_hello_world_timed.cha");
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    let timings = vec![
        Some(WordTiming {
            start_ms: 100,
            end_ms: 500,
        }),
        Some(WordTiming {
            start_ms: 600,
            end_ms: 1000,
        }),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);
    assert_eq!(offset, 2);

    let utt = get_test_utterance(&mut chat, 0);
    let items = &utt.main.content.content;
    match &items[0] {
        UtteranceContent::Word(w) => {
            assert!(
                w.inline_bullet.is_some(),
                "Expected inline_bullet to be set"
            );
        }
        _ => panic!("Expected word"),
    }
}

#[test]
fn test_fa_cache_key() {
    let words = vec!["hello".to_string(), "world".to_string()];
    let key = cache_key(
        &words,
        &AudioIdentity::from_metadata("test.mp3", 1234, 5678),
        0,
        5000,
        FaTimingMode::WithPauses,
        FaEngineType::WhisperFa,
    );
    // Verify it's a valid hex BLAKE3 (64 chars)
    assert_eq!(key.as_str().len(), 64);
    assert!(key.as_str().chars().all(|c| c.is_ascii_hexdigit()));

    // Same inputs -> same key
    let key2 = cache_key(
        &words,
        &AudioIdentity::from_metadata("test.mp3", 1234, 5678),
        0,
        5000,
        FaTimingMode::WithPauses,
        FaEngineType::WhisperFa,
    );
    assert_eq!(key, key2);

    // Different timing mode -> different key
    let key3 = cache_key(
        &words,
        &AudioIdentity::from_metadata("test.mp3", 1234, 5678),
        0,
        5000,
        FaTimingMode::Continuous,
        FaEngineType::WhisperFa,
    );
    assert_ne!(key, key3);
}

#[test]
fn test_apply_fa_results() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_hello_world_goodbye_timed.cha");
    let mut chat = parse_chat(input);

    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(0, 10000),
        words: vec![
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(0),
                text: "hello".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(1),
                text: "world".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(1),
                utterance_word_index: WordIdx(0),
                text: "goodbye".into(),
            },
        ],
        utterance_indices: vec![UtteranceIdx(0), UtteranceIdx(1)],
    }];

    let responses = vec![vec![
        Some(WordTiming {
            start_ms: 100,
            end_ms: 1000,
        }),
        Some(WordTiming {
            start_ms: 1500,
            end_ms: 3000,
        }),
        Some(WordTiming {
            start_ms: 5500,
            end_ms: 8000,
        }),
    ]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        true,
    );

    let output = chat.to_chat_string();
    assert!(output.contains("%wor:"), "Output should contain %wor tier");
}

#[test]
fn test_monotonicity_enforcement() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_non_monotonic_bullets.cha");
    let mut chat = parse_chat(input);
    let decisions = enforce_monotonicity(&mut chat);

    // Second utterance (start=2000) is before first (start=5000) -- should be stripped
    let utt = get_test_utterance(&mut chat, 1);
    assert!(
        utt.main.content.bullet.is_none(),
        "Non-monotonic utterance should have timing stripped"
    );

    // Should produce a decision record for the stripped utterance
    assert_eq!(
        decisions.len(),
        1,
        "should have 1 decision for stripped utterance"
    );
    assert_eq!(decisions[0].strategy.strategy_name(), "timing_stripped");
    assert!(decisions[0].needs_review);
}

#[test]
fn test_monotonicity_clamps_overlapping_end_times() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_overlapping_end_times.cha");
    let mut chat = parse_chat(input);
    let decisions = enforce_monotonicity(&mut chat);

    // Utterance 0: start=1000, original end=5000, next start=4000 → clamped to 4000
    let utt0 = get_test_utterance(&mut chat, 0);
    let b0 = utt0
        .main
        .content
        .bullet
        .as_ref()
        .expect("utt0 should keep timing");
    assert_eq!(
        b0.timing.end_ms, 4000,
        "utt0 end should be clamped to utt1 start"
    );

    // Utterance 1: start=4000, original end=8000, next start=7000 → clamped to 7000
    let utt1 = get_test_utterance(&mut chat, 1);
    let b1 = utt1
        .main
        .content
        .bullet
        .as_ref()
        .expect("utt1 should keep timing");
    assert_eq!(
        b1.timing.end_ms, 7000,
        "utt1 end should be clamped to utt2 start"
    );

    // Utterance 2: last utterance, no successor → end unchanged at 12000
    let utt2 = get_test_utterance(&mut chat, 2);
    let b2 = utt2
        .main
        .content
        .bullet
        .as_ref()
        .expect("utt2 should keep timing");
    assert_eq!(b2.timing.end_ms, 12000, "last utt end should be unchanged");

    // Should produce 2 end_clamped decisions (utt0→utt1, utt1→utt2)
    let clamp_decisions: Vec<_> = decisions
        .iter()
        .filter(|d| d.strategy.strategy_name() == "end_clamped")
        .collect();
    assert_eq!(
        clamp_decisions.len(),
        2,
        "should have 2 end_clamped decisions"
    );
    // `end_clamped` is routine housekeeping — a few-millisecond UTR overlap
    // correction.  It must NOT set needs_review because that writes %xrev: [?],
    // causing CLAN to flag a correctly-aligned utterance for human review.
    // BA2 made these same adjustments silently.  Only `timing_stripped` (where
    // the utterance lost all timing) deserves a human review flag.
    //
    // CURRENTLY RED: needs_review is true, writing %xrev on every trimmed utt.
    // AFTER FIX: needs_review is false; %xalign is still written (audit log)
    // but no %xrev appears.
    assert!(
        !clamp_decisions[0].needs_review,
        "end_clamped must NOT need review — it is routine overlap correction, \
         not an alignment defect requiring human inspection"
    );
}

#[test]
fn test_parse_fa_response_token_level() {
    let json = r#"{"tokens": [
            {"text": "hello", "time_s": 0.1},
            {"text": "world", "time_s": 0.6}
        ]}"#;
    let words = make_fa_words(&["hello", "world"]);
    let timings = parse_fa_response(json, &words, 0, FaTimingMode::Continuous).unwrap();
    assert_eq!(timings.len(), 2);
    assert_eq!(
        timings[0],
        Some(WordTiming {
            start_ms: 100,
            end_ms: 100
        })
    );
    assert_eq!(
        timings[1],
        Some(WordTiming {
            start_ms: 600,
            end_ms: 600
        })
    );
}

#[test]
fn test_parse_fa_response_token_level_punctuation_token_is_ignored() {
    let json = r#"{"tokens": [
            {"text": "hello", "time_s": 0.1},
            {"text": ",", "time_s": 0.2},
            {"text": "world", "time_s": 0.6}
        ]}"#;
    let words = make_fa_words(&["hello", "world"]);
    let timings = parse_fa_response(json, &words, 3000, FaTimingMode::Continuous).unwrap();
    assert_eq!(
        timings[0],
        Some(WordTiming {
            start_ms: 3100,
            end_ms: 3100
        })
    );
    assert_eq!(
        timings[1],
        Some(WordTiming {
            start_ms: 3600,
            end_ms: 3600
        })
    );
}

#[test]
fn test_parse_fa_response_token_level_mismatch_does_not_skip_tokens() {
    let json = r#"{"tokens": [
            {"text": "hello", "time_s": 0.1},
            {"text": "there", "time_s": 0.2},
            {"text": "world", "time_s": 0.6}
        ]}"#;
    let words = make_fa_words(&["hello", "world"]);
    let timings = parse_fa_response(json, &words, 0, FaTimingMode::Continuous).unwrap();
    assert_eq!(
        timings[0],
        Some(WordTiming {
            start_ms: 100,
            end_ms: 100
        })
    );
    assert_eq!(timings[1], None);
}

#[test]
fn test_parse_fa_response_indexed_word_level() {
    let json = r#"{"indexed_timings": [
            {"start_ms": 100, "end_ms": 500},
            {"start_ms": 600, "end_ms": 1000}
        ]}"#;
    let words = make_fa_words(&["hello", "world"]);
    let timings = parse_fa_response(json, &words, 5000, FaTimingMode::Continuous).unwrap();
    assert_eq!(timings.len(), 2);
    assert_eq!(timings[0].as_ref().unwrap().start_ms, 5100);
    assert_eq!(timings[0].as_ref().unwrap().end_ms, 5500);
    assert_eq!(timings[1].as_ref().unwrap().start_ms, 5600);
    assert_eq!(timings[1].as_ref().unwrap().end_ms, 6000);
}

#[test]
fn test_parse_fa_response_indexed_length_mismatch_rejected() {
    use crate::chat_ops::fa::alignment::FaAlignmentError;
    let json = r#"{"indexed_timings": [{"start_ms": 100, "end_ms": 500}]}"#;
    let words = make_fa_words(&["hello", "world"]);
    let err = parse_fa_response(json, &words, 0, FaTimingMode::Continuous).unwrap_err();
    // Wave 5 consolidation: typed error replaces the previous stringly
    // "length mismatch" substring check. Assert on the variant shape so
    // a refactor that re-introduces a stringly path fails loudly.
    match err {
        FaAlignmentError::IndexedCountMismatch { expected, actual } => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("expected IndexedCountMismatch, got {other:?}"),
    }
}

#[test]
fn test_estimate_boundaries_proportional() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_two_untimed_with_media.cha");
    let chat = parse_chat(input);
    let estimates = estimate_untimed_boundaries(&chat, 10000);
    assert_eq!(estimates.len(), 2);
    assert_eq!(estimates[0].start_ms, 0);
    assert_eq!(estimates[0].end_ms, 7000);
    assert_eq!(estimates[1].start_ms, 3000);
    assert_eq!(estimates[1].end_ms, 10000);
}

#[test]
fn test_estimate_boundaries_interpolates_from_neighbors() {
    let input =
        include_str!("../../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");
    let chat = parse_chat(input);
    let estimates = estimate_untimed_boundaries(&chat, 50000);

    // 6 utterances total
    assert_eq!(estimates.len(), 6);

    // utt 0: timed (10000-15000), estimate mirrors real bullet
    assert_eq!(estimates[0], TimeSpan::new(10000, 15000));

    // utt 1: untimed, between timed utt 0 (end=15000) and utt 2 (start=20000)
    // Gap = [15000, 20000], 4 words, only utterance in run
    // raw: 15000-20000, with 2s buffer: 13000-22000
    assert_eq!(estimates[1].start_ms, 13000);
    assert_eq!(estimates[1].end_ms, 22000);

    // utt 2: timed (20000-25000)
    assert_eq!(estimates[2], TimeSpan::new(20000, 25000));

    // utt 3: untimed, in run [3,4] between timed utt 2 (end=25000) and utt 5 (start=40000)
    // Gap = [25000, 40000] = 15000ms, run_words = 4+5 = 9
    // utt 3 (4 words): raw 25000..31666, buffered 23000..33666
    assert_eq!(estimates[3].start_ms, 23000);
    assert_eq!(estimates[3].end_ms, 33666);

    // utt 4 (5 words): raw 31666..40000, buffered 29666..42000
    assert_eq!(estimates[4].start_ms, 29666);
    assert_eq!(estimates[4].end_ms, 42000);

    // utt 5: timed (40000-45000)
    assert_eq!(estimates[5], TimeSpan::new(40000, 45000));
}

#[test]
fn snapshot_fa_infer_item() {
    let item = FaInferItem {
        words: vec!["hello".into(), "world".into()],
        word_ids: vec!["u0:w0".into(), "u0:w1".into()],
        word_utterance_indices: vec![0, 0],
        word_utterance_word_indices: vec![0, 1],
        audio_path: "/data/test.mp3".into(),
        audio_start_ms: 1500,
        audio_end_ms: 3200,
        timing_mode: FaTimingMode::WithPauses,
    };
    insta::assert_json_snapshot!(item);
}

#[test]
fn test_apply_fa_results_excludes_xxx_from_wor_tier() {
    // Fixture: an utterance with `xxx`. The stale %wor has 5 words (no xxx) —
    // this matches what the new policy will produce after FA.
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "@Participants:\tINV Investigator\n",
        "@ID:\teng|test|INV|||||Investigator|||\n",
        "*INV:\tlast time I saw you xxx . \u{0015}27602_28323\u{0015}\n",
        "%wor:\tlast \u{0015}27602_27762\u{0015} time \u{0015}27762_27942\u{0015} I \u{0015}27942_28002\u{0015} saw \u{0015}28002_28203\u{0015} you \u{0015}28203_28323\u{0015} .\n",
        "@End\n",
    );
    let mut chat = parse_chat(input);

    // FA group: 5 words extracted by collect_fa_words — xxx is excluded because
    // untranscribed tokens have no alignable phoneme sequence.
    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(27602, 28323),
        words: vec![
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(0),
                text: "last".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(1),
                text: "time".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(2),
                text: "I".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(3),
                text: "saw".into(),
            },
            FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(4),
                text: "you".into(),
            },
            // xxx is NOT in the FA group — not sent to the aligner.
        ],
        utterance_indices: vec![UtteranceIdx(0)],
    }];

    // FA response: 5 timings for the 5 real words.
    let responses = vec![vec![
        Some(WordTiming {
            start_ms: 27602,
            end_ms: 27762,
        }),
        Some(WordTiming {
            start_ms: 27762,
            end_ms: 27942,
        }),
        Some(WordTiming {
            start_ms: 27942,
            end_ms: 28002,
        }),
        Some(WordTiming {
            start_ms: 28002,
            end_ms: 28203,
        }),
        Some(WordTiming {
            start_ms: 28203,
            end_ms: 28323,
        }),
    ]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        true,
    );

    let output = chat.to_chat_string();

    // The %wor tier must have 5 word entries — `xxx` is excluded, no slot for it.
    let post_wor: Vec<_> = get_utterance(&chat, 0)
        .wor_tier()
        .expect("output must contain a %wor tier after FA")
        .words()
        .map(|w| w.cleaned_text().to_string())
        .collect();
    assert_eq!(
        post_wor,
        vec!["last", "time", "I", "saw", "you"],
        "%wor tier must contain only the 5 real words (xxx excluded); \
         got: {post_wor:?}\nFull output:\n{output}"
    );

    // All 5 real words must have timing bullets.
    let wor = get_utterance(&chat, 0)
        .wor_tier()
        .expect("output must contain a %wor tier after FA");
    let wor_words: Vec<_> = wor.words().collect();
    assert!(
        wor_words[0].inline_bullet.is_some(),
        "`last` must have a timing bullet"
    );
    assert!(
        wor_words[4].inline_bullet.is_some(),
        "`you` must have a timing bullet"
    );
}
