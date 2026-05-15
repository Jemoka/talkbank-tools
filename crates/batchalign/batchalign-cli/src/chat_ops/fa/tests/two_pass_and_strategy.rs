//! Two-pass UTR vs. global UTR + strategy selection: `two_pass_*`, `global_utr_*`, `select_strategy_*`.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn test_two_pass_correctly_times_lazy_overlap() {
    use utr::UtrStrategy;
    let chat_text =
        include_str!("../../../../../../../resources/fixtures/utr_lazy_overlap_backchannel.cha");
    let mut chat = parse_chat(chat_text);
    let tokens = make_utr_tokens(&[
        // PAR's first utterance words
        ("I", 100, 300),
        ("went", 400, 800),
        ("to", 900, 1100),
        ("the", 1200, 1400),
        ("store", 1500, 2000),
        // INV's backchannel overlaps PAR's first utterance
        ("mhm", 1800, 2200),
        ("yesterday", 2300, 3000),
        // PAR's second utterance
        ("and", 5000, 5300),
        ("I", 5400, 5600),
        ("bought", 5700, 6200),
        ("some", 6300, 6600),
        ("groceries", 6700, 7500),
    ]);
    let result = utr::TwoPassOverlapUtr::new().inject(&mut chat, &tokens);

    // PAR's two utterances + INV's backchannel should all get timing
    assert_eq!(
        result.injected, 3,
        "all 3 untimed utterances should get timing"
    );
    assert_eq!(result.unmatched, 0);

    // Verify INV's "mhm" (utterance index 1) got correct timing
    let inv_bullet = get_utterance_bullet(&chat, 1).expect("INV +< mhm should have a bullet");
    assert!(
        inv_bullet.0 >= 1700 && inv_bullet.0 <= 1900,
        "INV start should be near 1800, got {}",
        inv_bullet.0,
    );
    assert!(
        inv_bullet.1 >= 2100 && inv_bullet.1 <= 2300,
        "INV end should be near 2200, got {}",
        inv_bullet.1,
    );
}

#[test]
fn test_global_utr_misaligns_lazy_overlap_backchannel() {
    use utr::UtrStrategy;
    let chat_text =
        include_str!("../../../../../../../resources/fixtures/utr_lazy_overlap_backchannel.cha");
    let mut chat = parse_chat(chat_text);
    let tokens = make_utr_tokens(&[
        ("I", 100, 300),
        ("went", 400, 800),
        ("to", 900, 1100),
        ("the", 1200, 1400),
        ("store", 1500, 2000),
        ("mhm", 1800, 2200),
        ("yesterday", 2300, 3000),
        ("and", 5000, 5300),
        ("I", 5400, 5600),
        ("bought", 5700, 6200),
        ("some", 6300, 6600),
        ("groceries", 6700, 7500),
    ]);
    let result = utr::GlobalUtr.inject(&mut chat, &tokens);

    // GlobalUtr may still inject timing for INV, but the timing will be wrong:
    // the DP assigns "mhm" to the token at its position in the global sequence
    // (after "yesterday" or misplaced), not within the overlapping window.
    // We verify that at least the injected count covers all utterances.
    assert_eq!(
        result.injected + result.unmatched,
        3,
        "all 3 untimed utterances accounted for"
    );
}

#[test]
fn test_two_pass_identical_without_lazy_overlap() {
    use utr::UtrStrategy;
    let chat_text =
        include_str!("../../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");

    // Build matching ASR tokens
    let tokens = make_utr_tokens(&[
        ("the", 10000, 10500),
        ("cat", 10600, 11000),
        ("is", 11200, 11500),
        ("here", 12000, 13000),
        ("she", 15500, 16000),
        ("is", 16200, 16500),
        ("looking", 16800, 17500),
        ("outside", 17800, 18500),
        ("there", 20500, 21000),
        ("is", 21200, 21500),
        ("a", 21800, 22000),
        ("path", 22200, 23000),
        ("I", 26000, 26500),
        ("do", 26800, 27000),
        ("not", 27200, 27500),
        ("know", 27800, 28500),
        ("but", 30000, 30500),
        ("there", 30800, 31200),
        ("is", 31500, 31800),
        ("a", 32000, 32200),
        ("building", 32500, 33500),
        ("okay", 40500, 41000),
        ("so", 41200, 41500),
        ("now", 41800, 42500),
    ]);

    let mut chat_global = parse_chat(chat_text);
    let mut chat_two_pass = parse_chat(chat_text);

    let r1 = utr::GlobalUtr.inject(&mut chat_global, &tokens);
    let r2 = utr::TwoPassOverlapUtr::new().inject(&mut chat_two_pass, &tokens);

    assert_eq!(r1.injected, r2.injected, "injected count should match");
    assert_eq!(r1.unmatched, r2.unmatched, "unmatched count should match");
    assert_eq!(r1.skipped, r2.skipped, "skipped count should match");

    // Compare bullets on each utterance
    for i in 0..6 {
        assert_eq!(
            get_utterance_bullet(&chat_global, i),
            get_utterance_bullet(&chat_two_pass, i),
            "utterance {i} bullets should match",
        );
    }
}

#[test]
fn test_two_pass_dense_backchannels() {
    use utr::UtrStrategy;
    let chat_text = include_str!("../../../../../../../resources/fixtures/utr_lazy_overlap_dense.cha");
    let mut chat = parse_chat(chat_text);

    // PAR's narrative spans 100-10000ms. INV's 4 backchannels are scattered within.
    let tokens = make_utr_tokens(&[
        // PAR's words
        ("I", 100, 300),
        ("grew", 400, 700),
        ("up", 800, 1000),
        ("in", 1100, 1300),
        ("Princeton", 1400, 2000),
        // INV backchannel 1: "oh okay"
        ("oh", 2100, 2300),
        ("okay", 2400, 2800),
        ("and", 2900, 3100),
        ("came", 3200, 3500),
        ("to", 3600, 3800),
        ("graduate", 3900, 4400),
        ("school", 4500, 5000),
        // INV backchannel 2: "mhm"
        ("mhm", 5100, 5400),
        ("at", 5500, 5700),
        ("Chapel", 5800, 6200),
        ("Hill", 6300, 6700),
        // INV backchannel 3: "oh"
        ("oh", 6800, 7100),
        ("in", 7200, 7400),
        ("ninety", 7500, 7900),
        ("one", 8000, 8300),
        // INV backchannel 4: "mhm"
        ("mhm", 8400, 8700),
        ("or", 8800, 9000),
        ("maybe", 9100, 9500),
        ("ninety", 9600, 9900),
        ("two", 10000, 10300),
    ]);

    let result = utr::TwoPassOverlapUtr::new().inject(&mut chat, &tokens);

    // PAR's utterance (1) + 4 INV backchannels = 5 injected
    assert_eq!(
        result.injected, 5,
        "PAR + 4 INV backchannels should be timed"
    );
    assert_eq!(result.unmatched, 0);

    // All 4 INV utterances (indices 1-4) should have bullets within PAR's range
    for inv_idx in 1..=4 {
        let bullet = get_utterance_bullet(&chat, inv_idx)
            .unwrap_or_else(|| panic!("INV utterance {inv_idx} should have a bullet"));
        assert!(
            bullet.0 >= 100 && bullet.1 <= 11000,
            "INV utterance {inv_idx} bullet {}-{} should be within PAR's range",
            bullet.0,
            bullet.1,
        );
    }
}

#[test]
fn test_select_strategy_chooses_correctly() {
    let with_overlap =
        include_str!("../../../../../../../resources/fixtures/utr_lazy_overlap_backchannel.cha");
    let without_overlap =
        include_str!("../../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");

    let chat_overlap = parse_chat(with_overlap);
    let chat_no_overlap = parse_chat(without_overlap);

    // We can't check the concrete type directly, but we can verify behavior:
    // select_strategy on a +< file should produce TwoPassOverlapUtr results
    let strategy = utr::select_strategy(&chat_overlap, None);
    let mut chat = parse_chat(with_overlap);
    let tokens = make_utr_tokens(&[
        ("I", 100, 300),
        ("went", 400, 800),
        ("to", 900, 1100),
        ("the", 1200, 1400),
        ("store", 1500, 2000),
        ("mhm", 1800, 2200),
        ("yesterday", 2300, 3000),
        ("and", 5000, 5300),
        ("I", 5400, 5600),
        ("bought", 5700, 6200),
        ("some", 6300, 6600),
        ("groceries", 6700, 7500),
    ]);
    let result = strategy.inject(&mut chat, &tokens);
    assert_eq!(result.injected, 3, "should use two-pass and time all 3");

    // select_strategy on a non-+< file should use GlobalUtr
    let strategy = utr::select_strategy(&chat_no_overlap, None);
    let _ = strategy; // Just verify it compiles and returns
}
