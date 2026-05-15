//! Rerun/repair bullet behaviors: stale-x-tier cleanup, FA→UTR bullet overwriting, zero-duration authoritative-bullet clearing, backward-timestamp stripping, fast-path resilience.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_rerun_fa_strips_stale_x_tiers_even_when_no_new_decisions() {
    // A pre-aligned file with existing %xalign / %xrev from a previous run.
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\thello world . \u{0015}1000_3000\u{0015}\n\
%xalign:\tfa:old_decision old_reason_from_previous_run\n\
%xrev:\t[ok]\n\
@End\n\
";
    let mut chat = parse_chat(input);

    // Re-run: apply FA with clean word timings (no decisions expected).
    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(0, 5000),
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
        ],
        utterance_indices: vec![UtteranceIdx(0)],
    }];
    let responses = vec![vec![
        Some(WordTiming {
            start_ms: 1000,
            end_ms: 1500,
        }),
        Some(WordTiming {
            start_ms: 1500,
            end_ms: 3000,
        }),
    ]];
    let decisions = apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        false,
    );

    // Simulate fa/mod.rs step 9d — the BUGGY path: only injects (and strips)
    // when decisions is non-empty.  Clean re-run → decisions is empty → no
    // strip → old tiers remain.
    if !decisions.is_empty() {
        talkbank_transform::decisions::inject_decision_tiers(
            &mut chat,
            &decisions,
            crate::chat_ops::fa::ReviewLevel::LowConfidence,
        );
    }

    let output = chat.to_chat_string();
    let xalign_count = output.matches("%xalign:").count();
    let xrev_count = output.matches("%xrev:").count();

    // After a clean re-run, ALL old %xalign and %xrev tiers must be gone — even
    // when no new decisions were made.  Leaving stale tiers from the previous
    // run means the NEXT run that produces decisions will append to them,
    // producing duplicates.
    assert_eq!(
        xalign_count, 0,
        "stale %xalign from previous run must be stripped on re-run even with no new decisions; \
         got {xalign_count}:\n{output}"
    );
    assert_eq!(
        xrev_count, 0,
        "stale %xrev from previous run must be stripped on re-run even with no new decisions; \
         got {xrev_count}:\n{output}"
    );
}

#[test]
fn test_fa_bullet_overwrites_utr_hint_with_word_derived_timing() {
    use talkbank_model::model::BulletSource;

    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\thello world . \u{0015}800_3000\u{0015}\n\
@End\n\
";
    let mut chat = parse_chat(input);

    // Simulate what UTR does at runtime: mark the bullet as a provisional hint
    // so that update_utterance_bullet knows to overwrite it after FA.
    {
        let utt = get_test_utterance(&mut chat, 0);
        let bullet = utt
            .main
            .content
            .bullet
            .as_mut()
            .expect("test requires pre-existing UTR bullet");
        bullet.source = BulletSource::Utr;
    }

    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(800, 3000),
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
        ],
        utterance_indices: vec![UtteranceIdx(0)],
    }];

    let responses = vec![vec![
        Some(WordTiming::new(1000, 1500)),
        Some(WordTiming::new(1500, 2000)),
    ]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        false,
    );

    let (start, end) =
        get_utterance_bullet(&chat, 0).expect("utterance must have a bullet after FA");
    assert_eq!(
        start, 1000,
        "FA word span must overwrite UTR hint start: expected 1000, got {start}. \
         UTR hint was 800 but FA aligned first word to 1000."
    );
    assert_eq!(
        end, 2000,
        "FA word span must overwrite UTR hint end: expected 2000, got {end}. \
         UTR hint was 3000 but FA aligned last word to 2000."
    );
}

#[test]
fn test_fa_preserves_utr_hint_when_all_words_untimed() {
    use talkbank_model::model::BulletSource;

    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\thello world . \u{0015}1000_3000\u{0015}\n\
@End\n\
";
    let mut chat = parse_chat(input);

    // Simulate UTR having set this bullet as a provisional hint.
    {
        let utt = get_test_utterance(&mut chat, 0);
        let bullet = utt
            .main
            .content
            .bullet
            .as_mut()
            .expect("test requires pre-existing UTR bullet");
        bullet.source = BulletSource::Utr;
    }

    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(1000, 3000),
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
        ],
        utterance_indices: vec![UtteranceIdx(0)],
    }];

    // FA total failure: all words return None.
    let responses = vec![vec![None, None]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        false,
    );

    let (start, end) = get_utterance_bullet(&chat, 0)
        .expect("UTR hint must survive when FA produced no word timings");
    assert_eq!(
        start, 1000,
        "UTR hint start must be preserved when FA produced no timings, got {start}"
    );
    assert_eq!(
        end, 3000,
        "UTR hint end must be preserved when FA produced no timings, got {end}"
    );
}

#[test]
fn test_rescued_rerun_bullet_does_not_clamp_new_fa_words() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\tokay \u{0015}2200_2300\u{0015} thank \u{0015}2300_2450\u{0015} you \u{0015}2450_2600\u{0015} . \u{0015}1000_1300\u{0015}\n\
%wor:\tokay \u{0015}1100_1200\u{0015} thank \u{0015}1200_1300\u{0015} you .\n\
*CHI:\tbye . \u{0015}3000_3200\u{0015}\n\
@End\n\
";
    let mut chat = parse_chat(input);

    let decisions = rescue_narrow_bullets(&mut chat);
    assert_eq!(decisions.len(), 1, "narrow-bullet rescue should fire");

    {
        let utt = get_test_utterance(&mut chat, 0);
        let bullet = utt
            .main
            .content
            .bullet
            .as_ref()
            .expect("rescued utterance should still have a bullet");
        assert_eq!(
            bullet.source,
            talkbank_model::model::BulletSource::Utr,
            "rescued bullet must stay provisional so postprocess will not clamp FA back into the stale narrow span",
        );

        let dropped = postprocess_utterance_timings(utt, FaTimingMode::WithPauses);
        assert_eq!(
            dropped, 0,
            "rescued provisional bullet must not drop new FA timings during rerun postprocess",
        );
        update_utterance_bullet(utt);
    }

    let output = chat.to_chat_string();
    assert!(
        output.contains("okay \u{15}2200_2300\u{15} thank \u{15}2300_2450\u{15}"),
        "rescued rerun should keep FA word timings beyond the original narrow bullet:\n{output}",
    );
}

#[test]
fn test_fa_sets_bullet_from_word_span_when_no_prior_bullet() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\thello world .\n\
@End\n\
";
    let mut chat = parse_chat(input);

    // No pre-existing bullet.
    assert!(
        get_utterance_bullet(&chat, 0).is_none(),
        "test requires utterance to have no bullet initially"
    );

    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(0, 5000),
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
        ],
        utterance_indices: vec![UtteranceIdx(0)],
    }];

    let responses = vec![vec![
        Some(WordTiming::new(1000, 1500)),
        Some(WordTiming::new(1500, 2000)),
    ]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        false,
    );

    let (start, end) =
        get_utterance_bullet(&chat, 0).expect("utterance must have a bullet after FA");
    assert_eq!(
        start, 1000,
        "bullet start must be first word start, got {start}"
    );
    assert_eq!(end, 2000, "bullet end must be last word end, got {end}");
}

#[test]
fn test_fa_clears_zero_duration_authoritative_bullet_when_fa_produces_no_word_timings() {
    // Simulate a file that had a zero-duration bullet (start == end) from a
    // previous buggy FA run. The bullet is parsed from the file, so it is
    // BulletSource::Authoritative (the default for Bullet::new / parsed bullets).
    let input = "\
@UTF8\n@Begin\n@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|2;0.0||||Target_Child|||\n\
*CHI:\tz@l . \u{0015}245986_245986\u{0015}\n\
@End\n";
    let mut chat = parse_chat(input);

    // Confirm the bullet is zero-duration before FA.
    let (pre_start, pre_end) =
        get_utterance_bullet(&chat, 0).expect("test setup: utterance must have a bullet before FA");
    assert_eq!(
        pre_start, pre_end,
        "test setup: bullet must be zero-duration"
    );

    // FA returns all None — e.g. the FA engine cannot align a single letter.
    let groups = vec![FaGroup {
        audio_span: TimeSpan::new(245000, 247000),
        words: vec![FaWord {
            utterance_index: UtteranceIdx(0),
            utterance_word_index: WordIdx(0),
            text: "z".into(),
        }],
        utterance_indices: vec![UtteranceIdx(0)],
    }];
    let responses = vec![vec![None]];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::WithPauses,
        false,
    );

    // The zero-duration bullet must be cleared, not preserved.
    assert!(
        get_utterance_bullet(&chat, 0).is_none(),
        "zero-duration authoritative bullet must be cleared when FA produces no word timings \
         (keeping it produces E362); found {:?}",
        get_utterance_bullet(&chat, 0)
    );
}

#[test]
fn test_fa_backward_timestamp_from_wrong_audio_window_is_stripped() {
    // Simulate a previously-aligned CHAT file (BA2 bullets already present).
    // Two consecutive same-speaker utterances:
    //   utt 0: INV "alright ."       correctly aligned at 731556_733418
    //   utt 1: INV "so take a look"  FA ran on the wrong earlier window → 639095_640375
    let chat_text = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "@Participants:\tINV Investigator Adult_Unrelated\n",
        "@ID:\teng|test|INV||female|||Adult_Unrelated|||\n",
        "@Media:\ttest, audio\n",
        "*INV:\talright . \u{15}731556_733418\u{15}\n",
        "*INV:\tso take a look at all of them . \u{15}639095_640375\u{15}\n",
        "@End\n",
    );

    let mut chat = parse_chat(chat_text);

    // FA groups: each utterance is its own group.
    // Group 0 (correct): "alright" aligned to audio at 731556–733418.
    // Group 1 (wrong window): "so take a look …" aligned to earlier window at
    //   639000–641000ms — FA returns timings relative to that wrong window.
    let groups = vec![
        FaGroup {
            audio_span: TimeSpan::new(731000, 735000),
            words: vec![FaWord {
                utterance_index: UtteranceIdx(0),
                utterance_word_index: WordIdx(0),
                text: "alright".into(),
            }],
            utterance_indices: vec![UtteranceIdx(0)],
        },
        FaGroup {
            audio_span: TimeSpan::new(637000, 645000),
            words: vec![
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(0),
                    text: "so".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(1),
                    text: "take".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(2),
                    text: "a".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(3),
                    text: "look".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(4),
                    text: "at".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(5),
                    text: "all".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(6),
                    text: "of".into(),
                },
                FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(7),
                    text: "them".into(),
                },
            ],
            utterance_indices: vec![UtteranceIdx(1)],
        },
    ];

    // FA responses: group 1 returns timings from the wrong window (backward
    // relative to group 0's correct 731556ms start).
    let responses = vec![
        // Group 0: "alright" — correct.
        vec![Some(WordTiming::new(731556, 733418))],
        // Group 1: wrong window — all timings < 731556ms.
        vec![
            Some(WordTiming::new(639095, 639300)),
            Some(WordTiming::new(639400, 639600)),
            Some(WordTiming::new(639700, 639850)),
            Some(WordTiming::new(639900, 640050)),
            Some(WordTiming::new(640050, 640150)),
            Some(WordTiming::new(640150, 640250)),
            Some(WordTiming::new(640250, 640310)),
            Some(WordTiming::new(640310, 640375)),
        ],
    ];

    apply_fa_results(
        &mut chat,
        &groups,
        &responses,
        FaTimingMode::Continuous,
        false,
    );
    enforce_monotonicity(&mut chat);

    // utt 1 starts at 639095ms < utt 0's 731556ms → non-monotonic → must be stripped.
    let utt1 = get_utterance(&chat, 1);
    assert!(
        utt1.main.content.bullet.is_none(),
        "backward-timestamp utterance (FA wrong audio window) must have bullet stripped; \
         got: {:?}",
        utt1.main.content.bullet
    );

    // utt 0 must be unaffected — it was correctly timed.
    let utt0 = get_utterance(&chat, 0);
    let b0 = utt0
        .main
        .content
        .bullet
        .as_ref()
        .expect("correctly-timed utterance 0 must keep its bullet");
    assert_eq!(b0.timing.start_ms, 731556, "utt 0 start must be 731556ms");
}

#[test]
fn test_fast_path_strips_backward_wor_timestamps_and_removes_stale_wor_tier() {
    use talkbank_model::model::DependentTier;

    // Two utterances: utt0 correct (731556ms), utt1 backward (639095ms < 733418ms).
    let chat_text = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "@Participants:\tINV Investigator Adult_Unrelated\n",
        "@ID:\teng|test|INV||female|||Adult_Unrelated|||\n",
        "@Media:\ttest, audio\n",
        "*INV:\talright .\n",
        "%wor:\talright \u{15}731556_733418\u{15} .\n",
        "*INV:\tlook .\n",
        "%wor:\tlook \u{15}639095_639300\u{15} .\n",
        "@End\n",
    );
    let mut chat = parse_chat(chat_text);

    // Fast path precondition: %wor must be reusable for all utterances.
    assert!(
        has_reusable_wor_timing(&chat),
        "precondition: %wor must be complete and reusable"
    );

    // Step 1 (fast path): reconstruct main-tier bullets from %wor.
    refresh_existing_alignment(&mut chat, true);

    // After reconstruction, utt0 has a forward bullet and utt1 has a backward
    // bullet (639095ms < utt0's end time 733418ms).
    let utt1_after_refresh = get_utterance(&chat, 1);
    assert!(
        utt1_after_refresh.main.content.bullet.is_some(),
        "refresh_existing_alignment must reconstruct a bullet from %wor; \
         without fix the fast path returns here with a backward bullet"
    );

    // Step 2 (fast path FIX): call enforce_monotonicity to strip backward bullets.
    let decisions = enforce_monotonicity(&mut chat);

    // Step 3 (fast path FIX): remove %wor from utterances whose bullets were
    // stripped, so the next re-run cannot reconstruct the backward bullet again.
    strip_wor_from_monotonicity_stripped_utterances(&mut chat, &decisions);

    let utt0 = get_utterance(&chat, 0);
    let utt1 = get_utterance(&chat, 1);

    // utt0 retains its forward bullet.
    let b0 = utt0
        .main
        .content
        .bullet
        .as_ref()
        .expect("utt0 must keep its forward bullet");
    assert_eq!(b0.timing.start_ms, 731556, "utt0 start must be 731556ms");

    // utt1's backward bullet must be stripped.
    assert!(
        utt1.main.content.bullet.is_none(),
        "backward utt1 bullet (639095ms < utt0 end {}ms) must be stripped by \
         enforce_monotonicity; got {:?}",
        b0.timing.end_ms,
        utt1.main.content.bullet,
    );

    // utt1's %wor tier must be removed so the next re-run cannot reconstruct
    // the backward bullet from stale %wor timing.  This is the cycle-breaker.
    let utt1_has_wor = utt1
        .dependent_tiers
        .iter()
        .any(|t| matches!(t, DependentTier::Wor(_)));
    assert!(
        !utt1_has_wor,
        "backward %wor tier must be removed from utt1 after bullet is stripped; \
         leaving stale backward %wor causes every re-run to reconstruct the \
         backward bullet perpetuating the E362 violation cycle"
    );
}
