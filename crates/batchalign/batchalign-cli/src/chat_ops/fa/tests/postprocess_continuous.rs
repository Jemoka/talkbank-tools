//! Continuous postprocess pass: gap-bridging refusals, near-zero rebalancing, %wor clamping behavior.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_postprocess_continuous_does_not_extend_word_across_implausibly_large_gap() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|||||Target_Child|||\n\
@Media:\ttest, audio\n\
*CHI:\talpha beta gamma . \u{0015}0_12000\u{0015}\n\
@End\n\
";
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    let timings = vec![
        Some(WordTiming::new(100, 200)),
        Some(WordTiming::new(300, 400)),
        Some(WordTiming::new(10_000, 10_100)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[1],
        Some(WordTiming::new(300, 400)),
        "continuous mode must not stretch beta across a multi-second internal gap"
    );
    assert_eq!(
        collected[2],
        Some(WordTiming::new(10_000, 10_100)),
        "the final word should retain its original non-zero duration"
    );
}

#[test]
fn test_postprocess_continuous_does_not_extend_word_across_one_second_internal_gap() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|||||Target_Child|||\n\
@Media:\ttest, audio\n\
*CHI:\tsorry keep going . \u{0015}4130_5860\u{0015}\n\
@End\n\
";
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    let timings = vec![
        Some(WordTiming::new(4265, 4465)),
        Some(WordTiming::new(5548, 5668)),
        Some(WordTiming::new(5688, 5908)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[0],
        Some(WordTiming::new(4265, 4465)),
        "continuous mode must not stretch sorry across a one-second internal gap before keep"
    );
    assert_eq!(
        collected[1],
        Some(WordTiming::new(5548, 5688)),
        "keep should retain its original pre-injection span"
    );
    assert_eq!(
        collected[2],
        Some(WordTiming::new(5688, 5908)),
        "going should retain its original non-zero duration"
    );
}

#[test]
fn test_postprocess_continuous_does_not_extend_compound_filler_across_following_gap() {
    let mut chat = parse_chat(&proof_chat(
        "+\" &-you_know that's not gonna happen . \u{0015}3287_5860\u{0015}",
    ));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-022: FA aligns the compound filler as
    // separate "you" / "know" words, injection merges them back into one CHAT
    // token, and continuous mode must not then stretch that merged filler
    // forward into the next lexical word.
    let timings = vec![
        Some(WordTiming::new(3664, 3744)),
        Some(WordTiming::new(4446, 4646)),
        Some(WordTiming::new(5027, 5247)),
        Some(WordTiming::new(5288, 5428)),
        Some(WordTiming::new(5448, 5588)),
        Some(WordTiming::new(5689, 5949)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[0],
        Some(WordTiming::new(3664, 4646)),
        "continuous mode must not stretch merged compound filler timing into the following word gap"
    );
    assert_eq!(
        collected[1],
        Some(WordTiming::new(5027, 5288)),
        "that's should retain its original pre-injection span"
    );
}

#[test]
fn test_postprocess_continuous_does_not_extend_lexical_word_across_following_filler_gap() {
    let mut chat = parse_chat(&proof_chat("he seems &-um tired ."));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-026: raw FA gives "seems" a normal span, then
    // continuous mode stretches it forward across the gap before a timed filler
    // word, making "seems" dominate the utterance.
    let timings = vec![
        Some(WordTiming::new(3383, 3464)),
        Some(WordTiming::new(3624, 4045)),
        Some(WordTiming::new(4527, 4687)),
        Some(WordTiming::new(4868, 5409)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[1],
        Some(WordTiming::new(3624, 4045)),
        "continuous mode must not stretch a lexical word across the following filler gap"
    );
    assert_eq!(
        collected[2],
        Some(WordTiming::new(4527, 4868)),
        "the filler may still extend to the following lexical word"
    );
}

#[test]
fn test_postprocess_continuous_does_not_extend_filler_across_gap_when_it_would_dominate_utterance()
{
    let mut chat = parse_chat(&proof_chat(
        "and &-um so , she goes to the ball . \u{0015}12990_15730\u{0015}",
    ));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-031: raw FA gives "um" a modest filler span,
    // then continuous mode stretches it across an internal silence up to "so",
    // making the filler dominate the short utterance. Keep the filler's own span
    // instead of smoothing it into a dominant token.
    let timings = vec![
        Some(WordTiming::new(12976, 13496)),
        Some(WordTiming::new(13597, 13997)),
        Some(WordTiming::new(14878, 14978)),
        Some(WordTiming::new(15139, 15219)),
        Some(WordTiming::new(15259, 15419)),
        Some(WordTiming::new(15419, 15479)),
        Some(WordTiming::new(15499, 15579)),
        Some(WordTiming::new(15599, 15759)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[1],
        Some(WordTiming::new(13597, 13997)),
        "continuous mode must not stretch a filler across a gap when the bridged filler span would dominate the utterance"
    );
    assert_eq!(
        collected[2],
        Some(WordTiming::new(14878, 15139)),
        "the following lexical word may still extend to the next lexical boundary"
    );
}

#[test]
fn test_postprocess_continuous_heals_near_zero_lexical_word_before_filler_when_not_dominant() {
    let mut chat = parse_chat(&proof_chat(
        "and I &-um put more butter in to make sure it gets nice and brown . \u{0015}4280_9540\u{0015}",
    ));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-027: raw FA leaves "I" at 20 ms before a
    // timed filler gap. Continuous mode should heal that near-zero lexical word
    // when extending it to the filler start still keeps the word well below the
    // dominance threshold for the utterance.
    let timings = vec![
        Some(WordTiming::new(4281, 4521)),
        Some(WordTiming::new(5022, 5042)),
        Some(WordTiming::new(6383, 6443)),
        Some(WordTiming::new(7123, 7263)),
        Some(WordTiming::new(7283, 7423)),
        Some(WordTiming::new(7444, 7684)),
        Some(WordTiming::new(7744, 7844)),
        Some(WordTiming::new(7904, 7984)),
        Some(WordTiming::new(8064, 8224)),
        Some(WordTiming::new(8244, 8404)),
        Some(WordTiming::new(8604, 8724)),
        Some(WordTiming::new(8925, 9125)),
        Some(WordTiming::new(9165, 9365)),
        Some(WordTiming::new(9385, 9465)),
        Some(WordTiming::new(9485, 9785)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[1],
        Some(WordTiming::new(5022, 6383)),
        "continuous mode should heal a near-zero lexical word by extending it to the following filler start when that bridge is not dominant"
    );
    assert_eq!(
        collected[2],
        Some(WordTiming::new(6383, 7123)),
        "the filler should still extend to the following lexical word"
    );
}

#[test]
fn test_postprocess_continuous_rebalances_near_zero_lexical_word_from_following_filler_span() {
    let mut chat = parse_chat(&proof_chat(
        "I'm not sure if that's a [/] &-um a boat that's &-like flipped up on the top of the car . \u{0015}4360_8520\u{0015}",
    ));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-028: raw FA already lands the first lexical
    // "a" exactly on the filler boundary at only 20 ms, while continuous mode
    // stretches the following filler forward into a much larger span. The
    // lexical word should reclaim enough time from that filler span to avoid a
    // near-zero collapse.
    let timings = vec![
        Some(WordTiming::new(4323, 4403)),
        Some(WordTiming::new(4443, 4563)),
        Some(WordTiming::new(4603, 4764)),
        Some(WordTiming::new(4764, 4804)),
        Some(WordTiming::new(4824, 5004)),
        Some(WordTiming::new(5224, 5244)),
        Some(WordTiming::new(5244, 5284)),
        Some(WordTiming::new(5565, 5585)),
        Some(WordTiming::new(5805, 6105)),
        Some(WordTiming::new(6266, 6506)),
        Some(WordTiming::new(6807, 7027)),
        Some(WordTiming::new(7087, 7347)),
        Some(WordTiming::new(7387, 7488)),
        Some(WordTiming::new(7708, 7788)),
        Some(WordTiming::new(7848, 7908)),
        Some(WordTiming::new(7988, 8189)),
        Some(WordTiming::new(8229, 8289)),
        Some(WordTiming::new(8309, 8389)),
        Some(WordTiming::new(8429, 8689)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[5],
        Some(WordTiming::new(5224, 5264)),
        "continuous mode should let a collapsed lexical word reclaim the minimum 40 ms from the following filler span"
    );
    assert_eq!(
        collected[6],
        Some(WordTiming::new(5264, 5565)),
        "the filler should keep the rest of its expanded span after lending 20 ms back to the lexical word"
    );
}

#[test]
fn test_postprocess_continuous_rebalances_near_zero_lexical_word_from_following_lexical_span() {
    let mut chat = parse_chat(&proof_chat(
        "and then I will put a thin layer of horseradish on one of the pieces , a thin layer of a Thousand Island dressing on the same piece . \u{0015}5310_14570\u{0015}",
    ));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-029: raw FA already leaves the second-clause
    // determiner "a" at 20 ms directly against the following lexical word
    // "Thousand". Continuous mode should rebalance that shared boundary so the
    // near-zero lexical word reaches the minimum duration floor instead of being
    // preserved as an implausible sliver.
    let timings = vec![
        Some(WordTiming::new(5381, 5501)),
        Some(WordTiming::new(5582, 5822)),
        Some(WordTiming::new(6202, 6222)),
        Some(WordTiming::new(6282, 6502)),
        Some(WordTiming::new(6842, 7082)),
        Some(WordTiming::new(7303, 7323)),
        Some(WordTiming::new(7623, 7823)),
        Some(WordTiming::new(7863, 8083)),
        Some(WordTiming::new(8103, 8143)),
        Some(WordTiming::new(8203, 8803)),
        Some(WordTiming::new(9284, 9364)),
        Some(WordTiming::new(9464, 9564)),
        Some(WordTiming::new(9584, 9624)),
        Some(WordTiming::new(9644, 9724)),
        Some(WordTiming::new(9784, 10264)),
        Some(WordTiming::new(10945, 10965)),
        Some(WordTiming::new(11405, 11585)),
        Some(WordTiming::new(11645, 11845)),
        Some(WordTiming::new(11865, 11925)),
        Some(WordTiming::new(11925, 11945)),
        Some(WordTiming::new(11945, 12305)),
        Some(WordTiming::new(12385, 12545)),
        Some(WordTiming::new(12565, 12926)),
        Some(WordTiming::new(13486, 13706)),
        Some(WordTiming::new(13786, 13886)),
        Some(WordTiming::new(13966, 14226)),
        Some(WordTiming::new(14266, 14687)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[19],
        Some(WordTiming::new(11925, 11965)),
        "continuous mode should let a collapsed lexical word reclaim the minimum 40 ms from the following lexical span"
    );
    assert_eq!(
        collected[20],
        Some(WordTiming::new(11965, 12385)),
        "the following lexical word should keep the rest of its span after lending 20 ms back to the collapsed word"
    );
}

#[test]
fn test_postprocess_continuous_rebalances_near_zero_lexical_word_from_preceding_filler_span() {
    let mut chat = parse_chat(&proof_chat(
        "and &-um I first let it cool a little bit . \u{0015}10510_14824\u{0015}",
    ));
    let utt = get_test_utterance(&mut chat, 0);

    // Trace from align-regression-030: raw FA leaves "I" at 20 ms after a
    // leading filler, and continuous mode expands that filler right up to the
    // collapsed lexical word. The lexical word should reclaim enough time back
    // from the preceding filler span to reach the minimum duration floor.
    let timings = vec![
        Some(WordTiming::new(10503, 10883)),
        Some(WordTiming::new(11063, 11303)),
        Some(WordTiming::new(11903, 11923)),
        Some(WordTiming::new(13184, 13584)),
        Some(WordTiming::new(13984, 14124)),
        Some(WordTiming::new(14124, 14164)),
        Some(WordTiming::new(14204, 14444)),
        Some(WordTiming::new(14504, 14524)),
        Some(WordTiming::new(14524, 14724)),
        Some(WordTiming::new(14744, 14824)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[1],
        Some(WordTiming::new(11063, 11883)),
        "continuous mode should let the preceding filler lend 20 ms back to a collapsed lexical word"
    );
    assert_eq!(
        collected[2],
        Some(WordTiming::new(11883, 11923)),
        "the collapsed lexical word should reclaim enough duration from the preceding filler span to reach the 40 ms floor"
    );
}

#[test]
fn test_postprocess_with_existing_wor_does_not_clamp_final_word_to_near_zero_duration() {
    let input = "\
@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|||||Target_Child|||\n\
@Media:\ttest, audio\n\
*CHI:\talpha beta gamma delta . \u{0015}0_1000\u{0015}\n\
%wor:\talpha \u{0015}100_200\u{0015} beta \u{0015}200_300\u{0015} gamma \u{0015}300_400\u{0015} delta \u{0015}400_500\u{0015} .\n\
@End\n\
";
    let mut chat = parse_chat(input);
    let utt = get_test_utterance(&mut chat, 0);

    let timings = vec![
        Some(WordTiming::new(100, 300)),
        Some(WordTiming::new(300, 500)),
        Some(WordTiming::new(500, 700)),
        Some(WordTiming::new(940, 1200)),
    ];
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::Continuous);
    assert_eq!(dropped, 0);
    update_utterance_bullet(utt);

    let mut collected = Vec::new();
    postprocess::collect_word_timings(&utt.main.content.content, &mut collected);

    assert_eq!(
        collected[3],
        Some(WordTiming::new(940, 1200)),
        "a rerun with existing %wor must keep the worker's final-word duration when clamping would collapse it to a near-zero tail"
    );
    let bullet = utt
        .main
        .content
        .bullet
        .as_ref()
        .expect("bullet should remain");
    assert_eq!(
        bullet.timing.end_ms, 1200,
        "utterance bullet end should expand to the healed final word end"
    );
}

#[test]
fn test_postprocess_does_not_clamp_word_timings_to_utr_hint_bullet() {
    use talkbank_model::alignment::helpers::{WordItemMut, walk_words_mut};
    use talkbank_model::model::{Bullet, BulletSource};

    // An utterance with a narrow UTR hint: 24905_25125 (220ms).
    // The real speech spans from ~24990 to ~27000ms — well beyond the hint.
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "@Participants:\tCHI Target_Child\n",
        "@ID:\teng|test|CHI||female|||Target_Child|||\n",
        "*CHI:\tooh that happened . \u{0015}24905_25125\u{0015}\n",
        "@End\n",
    );
    let mut chat = parse_chat(input);

    // Mark the bullet as a provisional UTR hint (simulating runtime UTR output).
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

    // Inject FA word timings that extend well beyond the UTR hint window.
    // "ooh"      : 24990-25200  (partially inside, partially outside 25125)
    // "that"     : 25200-26000  (entirely outside 25125 — currently DROPPED)
    // "happened" : 26000-27000  (entirely outside 25125 — currently DROPPED)
    {
        let utt = get_test_utterance(&mut chat, 0);
        let timings = [
            Some((24990u64, 25200u64)),
            Some((25200, 26000)),
            Some((26000, 27000)),
        ];
        let mut idx = 0;
        walk_words_mut(&mut utt.main.content.content, None, &mut |leaf| {
            if let WordItemMut::Word(w) = leaf {
                if let Some(Some((s, e))) = timings.get(idx) {
                    w.inline_bullet = Some(Bullet::new(*s, *e));
                }
                idx += 1;
            }
        });
    }

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::WithPauses);

    // CURRENTLY RED: 2 words ("that" and "happened") are dropped because
    // their timings (25200ms and 26000ms) exceed the UTR hint boundary 25125ms.
    // AFTER FIX: 0 words dropped — UTR hints must not gate word timing acceptance.
    assert_eq!(
        dropped, 0,
        "postprocess must not drop words when the utterance bullet is a provisional \
         UTR hint; 'that' (25200ms) and 'happened' (26000ms) fall outside the \
         220ms UTR window but are valid FA timings that must be preserved"
    );

    // Verify all 3 words have their original FA timings intact.
    let utt = get_utterance(&chat, 0);
    let mut word_timings: Vec<Option<(u64, u64)>> = Vec::new();
    walk_words(&utt.main.content.content, None, &mut |leaf| {
        if let talkbank_model::alignment::helpers::WordItem::Word(w) = leaf {
            word_timings.push(
                w.inline_bullet
                    .as_ref()
                    .map(|b| (b.timing.start_ms, b.timing.end_ms)),
            );
        }
    });
    assert_eq!(
        word_timings,
        vec![
            Some((24990, 25200)),
            Some((25200, 26000)),
            Some((26000, 27000)),
        ],
        "all 3 words must retain their FA-assigned timings after postprocess"
    );
}

#[test]
fn test_postprocess_does_not_clamp_word_timings_on_first_time_alignment_no_wor() {
    use talkbank_model::alignment::helpers::{WordItemMut, walk_words_mut};
    use talkbank_model::model::Bullet;

    // Utterance with a narrow ASR-derived bullet (220ms) and NO %wor tier.
    // This is the exact state produced by `transcribe` + `utseg` before the
    // first `align` run.  BulletSource is Authoritative (the default for
    // all parsed bullets — BulletSource is not persisted to CHAT text).
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "@Participants:\tPAR Participant\n",
        "@ID:\teng|test|PAR||female|||Participant|||\n",
        // narrow ASR bullet: 220ms for a ~3s sentence
        "*PAR:\tooh that happened . \u{0015}24905_25125\u{0015}\n",
        // no %wor tier — this is first-time alignment
        "@End\n",
    );
    let mut chat = parse_chat(input);

    // BulletSource is already Authoritative by default.
    // Confirm no %wor tier is present.
    {
        let utt = get_utterance(&chat, 0);
        assert!(
            utt.wor_tier().is_none(),
            "test precondition: utterance must have no %wor tier"
        );
    }

    // Inject FA word timings that extend well beyond the narrow ASR bullet.
    // "ooh"      : 24990-25200  (partially outside 25125ms boundary)
    // "that"     : 25200-26000  (entirely outside 25125ms — currently DROPPED)
    // "happened" : 26000-27000  (entirely outside 25125ms — currently DROPPED)
    {
        let utt = get_test_utterance(&mut chat, 0);
        let timings = [
            Some((24990u64, 25200u64)),
            Some((25200, 26000)),
            Some((26000, 27000)),
        ];
        let mut idx = 0;
        walk_words_mut(&mut utt.main.content.content, None, &mut |leaf| {
            if let WordItemMut::Word(w) = leaf {
                if let Some(Some((s, e))) = timings.get(idx) {
                    w.inline_bullet = Some(Bullet::new(*s, *e));
                }
                idx += 1;
            }
        });
    }

    let utt = get_test_utterance(&mut chat, 0);
    let dropped = postprocess_utterance_timings(utt, FaTimingMode::WithPauses);

    assert_eq!(
        dropped, 0,
        "first-time alignment (no %%wor tier): must not clamp FA word timings \
         to the ASR-derived utterance bullet; 'that' (25200ms) and 'happened' \
         (26000ms) are valid FA timings that must be preserved"
    );

    // Verify all 3 words have their original FA timings intact.
    let utt = get_utterance(&chat, 0);
    let mut word_timings: Vec<Option<(u64, u64)>> = Vec::new();
    walk_words(&utt.main.content.content, None, &mut |leaf| {
        if let WordItem::Word(w) = leaf {
            word_timings.push(
                w.inline_bullet
                    .as_ref()
                    .map(|b| (b.timing.start_ms, b.timing.end_ms)),
            );
        }
    });
    assert_eq!(
        word_timings,
        vec![
            Some((24990, 25200)),
            Some((25200, 26000)),
            Some((26000, 27000)),
        ],
        "all 3 words must retain their FA-assigned timings after postprocess"
    );
}
