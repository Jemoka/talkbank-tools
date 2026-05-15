//! Drift-class regression scenarios for UTR.
//!
//! These unit tests exercise the public [`super::inject_utr_timing`] entry
//! point against procedurally synthesized CHAT + ASR inputs that deterministically
//! reproduce the "monotonic DP drift" condition seen on real MICASE / biling /
//! rhd / samtale files. They are RED on the current `GlobalUtr`-backed pipeline
//! for at least the long-file + dense-overlap cases and are expected to turn
//! GREEN once Phase 3 of the segment-aware UTR plan flips `select_strategy` to
//! dispatch to `SegmentAwareUtr` — the same `inject_utr_timing` call is the
//! seam, so the tests are forward-compatible.
//!
//! # Invariants checked by [`run_and_check`]
//!
//! For every utterance that UTR assigned a bullet:
//!
//! 1. `end_ms > start_ms` (no zero- or negative-duration bullets).
//! 2. Adjacent **non-overlap** utterance bullets are strictly monotone in
//!    `start_ms`. Overlap-continuation utterances — those carrying a `+<`
//!    [`Linker::LazyOverlapPrecedes`] OR a `⌊` CA bottom-overlap marker —
//!    legitimately share timing with a predecessor and are EXCLUDED from the
//!    monotonicity chain (they neither participate in the comparison nor
//!    advance the `prev_start` cursor). This mirrors the overlap-aware pattern
//!    in the regression-harness arm for `UtteranceBulletMonotonicityPreserved`
//!    shipped with Task 1.1.
//! 3. Each assigned bullet lands inside the utterance's **ground-truth audio
//!    window**, expanded by one 500 ms word-cadence slack. This is the
//!    invariant that actually catches DP drift — the DP can remain
//!    monotone-preserving while still matching utterance K's words to tokens
//!    belonging to utterance K-3, producing a "wrong audio region" bullet
//!    that monotonicity alone cannot detect. Ground-truth windows are
//!    recovered from the synthetic cadence used in [`build_scenario`].
//!
//! # Scenario taxonomy
//!
//! See the segment-aware-UTR design notes (operator-local) Task 1.2.
//!
//! | Scenario                        | Convention        | Utts | Drives drift via                  |
//! |---------------------------------|-------------------|------|-----------------------------------|
//! | `drift_ca_overlap_long_file`    | `⌊⌋` + `⌈⌉`      | 500  | dense CA brackets + 10% missing   |
//! | `drift_lazy_overlap_long_file`  | `+<`              | 500  | transcript reordered vs ASR       |
//! | `drift_inline_backchannel_*`    | `&*SPK:`          | 500  | &* tokens absent from ASR stream  |
//! | `drift_mixed_conventions`       | all three         | 500  | combined                          |
//! | `short_file_baseline`           | `⌊⌋`             | 30   | sanity check: should align cleanly |
//! | `anchor_sparse_stopword_heavy`  | none              | 500  | transcript is ALL <4-char tokens  |

use crate::chat_ops::fa::utr::{AsrTimingToken, UtrResult, inject_utr_timing, overlap_markers};
use talkbank_model::model::{ChatFile, Line, Linker};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse::parse_lenient;

/// Ground-truth audio window for an utterance, recovered from the synthetic
/// cadence used by [`build_scenario`]. Drift detection compares the UTR-
/// assigned bullet against this envelope.
#[derive(Debug, Clone, Copy)]
struct ExpectedWindow {
    /// Retained for Debug output / future provenance diagnostics. By
    /// construction this equals the enclosing `Vec<ExpectedWindow>` index
    /// (see `build_scenario`); no runtime check reads it.
    #[allow(dead_code)]
    utt_index: usize,
    expected_start_ms: u64,
    expected_end_ms: u64,
}

/// Which overlap convention the synthesized CHAT document uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlapConvention {
    /// `⌈ … ⌉` top-overlap + `⌊ … ⌋` bottom-overlap — the CA bracket pair.
    CaBracket,
    /// `+<` lazy-overlap-precedes linker.
    LazyPrecedes,
    /// `&*SPK:word` inline backchannel tokens embedded in the main utterance.
    InlineBackchannel,
    /// All three conventions interleaved.
    Mixed,
    /// No overlap markers at all.
    None,
}

/// Knobs that control how a drift scenario's CHAT+ASR pair is built.
#[derive(Debug, Clone, Copy)]
struct DriftParams {
    /// Number of main-speaker utterances to emit.
    n_utts: usize,
    /// Overlap convention used when an utterance carries a marker.
    convention: OverlapConvention,
    /// Fraction of utterances (0.0–1.0) that carry an overlap marker.
    overlap_density: f64,
    /// Fraction of transcript words (0.0–1.0) that are DROPPED from the
    /// synthesized ASR stream, to emulate missed recognition.
    asr_missing_rate: f64,
    /// If true, every transcript word is a short stopword (< 4 chars), which
    /// tests the anchor-sparse / Option-4 fallback pathway.
    stopword_only: bool,
}

/// A single non-overlap utterance's witnessed start time, tagged with its
/// main-tier index so violation messages can point at the offending utterance.
#[derive(Debug, Clone, Copy)]
struct MonotoneProbe {
    utt_index: usize,
    start_ms: u64,
}

/// Shared-vocabulary high-frequency tokens. All speakers draw from this pool so
/// that vocabulary alone never disambiguates a match — the DP must rely on
/// temporal position, which is precisely what breaks under overlap.
const AMBIGUOUS_WORDS: &[&str] = &[
    "the", "and", "it", "is", "of", "to", "a", "that", "we", "he", "she", "they", "this", "was",
    "had",
];

/// All < 4 characters: stresses the anchor-sparse fallback pathway.
const STOPWORD_VOCAB: &[&str] = &["the", "a", "an", "it", "is", "of", "to", "in", "on"];

/// Anchor density: one anchor token (unique, > 4 chars) per this many words
/// across the document. Under GlobalUtr anchors don't help; under
/// SegmentAwareUtr they become segment checkpoints.
const ANCHOR_EVERY_N_WORDS: usize = 60;

/// Words per main-speaker utterance.
const WORDS_PER_UTT: usize = 6;

/// Words per backchannel utterance (InlineBackchannel only).
const BACKCHANNEL_WORDS: usize = 2;

/// Nominal duration of a main utterance in the synthetic timeline.
const UTT_DURATION_MS: u64 = 3000;

// Standard LCG multipliers used for deterministic pseudo-random draws in
// this test module. Different constants give statistically independent
// streams for density sampling vs. word selection vs. drop masking —
// matters because we want the overlap-pick, word-selection, and drop-mask
// decisions to be uncorrelated even though they share an input seed family.
const LCG_MULT_GLIBC: u64 = 1103515245; // glibc srand48 multiplier
const LCG_INC_GLIBC: u64 = 12345; // glibc srand48 increment
const LCG_MULT_KNUTH: u64 = 6364136223846793005; // Knuth MMIX multiplier
const LCG_MULT_FIB: u64 = 2654435761; // Fibonacci hashing constant
const LCG_MULT_PARK_MILLER: u64 = 48271; // Park-Miller minstd multiplier

/// How far into the host utterance a backchannel nest starts (relative to the
/// host's `start_ms`). Backchannels span ~400 ms inside a 3000 ms host.
const BACKCHANNEL_INSET_MS: u64 = 1200;
const BACKCHANNEL_SPAN_MS: u64 = 400;

/// Role a single utterance plays in the overlap layout. Drives both CHAT
/// markup and the monotonicity exclusion downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UttRole {
    /// Standalone utterance — no overlap markup.
    Solo,
    /// First member of a CA-bracket pair. Wraps head word in `⌈ … ⌉`.
    CaTop,
    /// Second member of a CA-bracket pair. Wraps head word in `⌊ … ⌋`.
    /// Counts as overlap-continuation for the monotonicity checker.
    CaBottom,
    /// First member of a lazy-overlap pair (no `+<` marker).
    LazyLeading,
    /// Second member of a lazy-overlap pair (carries `+<` linker).
    /// Counts as overlap-continuation for the monotonicity checker.
    LazyContinuation,
    /// Short backchannel nested inside a host utterance's time range.
    Backchannel,
}

/// A fully-laid-out utterance: a time range in the synthetic "audio" timeline,
/// the words that will be spoken, and the role it plays in the overlap layout.
///
/// Document order in the produced CHAT matches the order of this Vec. The
/// temporal order (ASR stream) is recovered by flattening per-word timings
/// across ALL layouts and re-sorting by `start_ms` — that re-sort is what
/// produces the interleaving that GlobalUtr's document-order DP cannot
/// reconcile.
#[derive(Debug, Clone)]
struct UttLayout {
    utt_index: usize,
    speaker: &'static str,
    role: UttRole,
    start_ms: u64,
    end_ms: u64,
    words: Vec<String>,
}

/// Geometry for a pair of adjacent overlapping utterances (CaBracket /
/// LazyPrecedes): the second utterance starts halfway through the first
/// and extends half a unit past its end, producing a 1500 ms intersection
/// for `UTT_DURATION_MS = 3000`.
///
/// Returns `(s0, e0, s1, e1)` where `s0 < s1 < e0 < e1`. Callers still decide
/// which speaker (MAI/OTH) and `UttRole` goes into each slot.
fn pair_overlap_ranges(cursor: u64) -> (u64, u64, u64, u64) {
    let s0 = cursor;
    let e0 = s0 + UTT_DURATION_MS * 3 / 2; // +4500
    let s1 = s0 + UTT_DURATION_MS / 2; // +1500
    let e1 = s1 + UTT_DURATION_MS * 3 / 2; // +6000 == s0 + 2*UTT_DURATION_MS
    (s0, e0, s1, e1)
}

/// Geometry for an inline backchannel: a short utterance whose time range
/// sits strictly inside a longer host utterance's range.
///
/// Returns `(host_s, host_e, bc_s, bc_e)` where
/// `host_s < bc_s < bc_e < host_e`. Uses the module-level
/// `BACKCHANNEL_INSET_MS` / `BACKCHANNEL_SPAN_MS` knobs so all tuning stays
/// in one place.
fn backchannel_ranges(cursor: u64) -> (u64, u64, u64, u64) {
    let host_s = cursor;
    let host_e = host_s + UTT_DURATION_MS;
    let bc_s = host_s + BACKCHANNEL_INSET_MS;
    let bc_e = bc_s + BACKCHANNEL_SPAN_MS;
    (host_s, host_e, bc_s, bc_e)
}

/// Step 1: place every utterance in time + assign a role. This is the heart of
/// the redesign — overlap conventions produce intersecting time ranges here,
/// and document-order CHAT emission downstream preserves the ordering that
/// GlobalUtr sees while the ASR re-sort breaks it.
fn layout_utterances_in_time(params: DriftParams) -> Vec<UttLayout> {
    // Deterministic Bernoulli: returns true with frequency ~= p, seeded by k.
    let overlap_picked = |k: usize, p: f64| -> bool {
        ((k as u64).wrapping_mul(LCG_MULT_PARK_MILLER) % 1000) < (p * 1000.0) as u64
    };

    let vocab: &[&str] = if params.stopword_only {
        STOPWORD_VOCAB
    } else {
        AMBIGUOUS_WORDS
    };

    // Utterance-index counter. Distinct from the main-timeline index `k`
    // because backchannels also consume utt indices while not advancing the
    // main timeline.
    let mut next_utt_idx: usize = 0;
    // Running anchor-slot counter, used to decide whether the j-th word is an
    // anchor (every Nth word across the document).
    let mut global_word_counter: usize = 0;
    // Running anchor-number counter, used to mint unique anchor tokens.
    let mut next_anchor_num: usize = 0;

    // Word-picker: closes over the anchor counters so anchors are globally
    // unique AND placed at the right sparse cadence. Stopword-only mode
    // disables anchors entirely.
    let mut mint_words = |seed: u64, count: usize| -> Vec<String> {
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let make_anchor = !params.stopword_only
                && global_word_counter > 0
                && global_word_counter.is_multiple_of(ANCHOR_EVERY_N_WORDS);
            global_word_counter += 1;
            if make_anchor {
                let w = format!("anchor{}", next_anchor_num);
                next_anchor_num += 1;
                out.push(w);
            } else {
                let mix = seed.wrapping_add((i as u64).wrapping_mul(LCG_MULT_KNUTH)) as usize;
                out.push(vocab[mix % vocab.len()].to_string());
            }
        }
        out
    };

    let mut layouts: Vec<UttLayout> = Vec::new();
    // Main-timeline cursor advances one `UTT_DURATION_MS` per main utterance.
    // Overlap pairs under CaBracket/LazyPrecedes still consume 2 main slots of
    // timeline each, just with intersecting intervals; backchannels nest
    // inside their host's range and do NOT advance the cursor.
    let mut cursor_ms: u64 = 0;
    let mut k: usize = 0;
    while k < params.n_utts {
        let seed = (k as u64).wrapping_mul(LCG_MULT_FIB);
        let remaining = params.n_utts - k;

        // Resolve a "pair-style" convention (two overlapping utts) for this k.
        // Mixed rotates through the three overlap styles.
        let effective_conv = match params.convention {
            OverlapConvention::Mixed => match k % 3 {
                0 => OverlapConvention::CaBracket,
                1 => OverlapConvention::LazyPrecedes,
                _ => OverlapConvention::InlineBackchannel,
            },
            other => other,
        };
        // Overall overlap density applies uniformly; Mixed rotates through
        // style at each k. (If we wanted style-specific densities later we
        // could branch here.)
        let wants_overlap_here = overlap_picked(k, params.overlap_density);

        match (effective_conv, wants_overlap_here, remaining >= 2) {
            (OverlapConvention::CaBracket, true, true) => {
                // Pair: two utts whose ranges intersect by 1500 ms.
                let (s0, e0, s1, e1) = pair_overlap_ranges(cursor_ms);
                let words0 = mint_words(seed, WORDS_PER_UTT);
                let words1 = mint_words(seed ^ 0xdead_beef, WORDS_PER_UTT);
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "MAI",
                    role: UttRole::CaTop,
                    start_ms: s0,
                    end_ms: e0,
                    words: words0,
                });
                next_utt_idx += 1;
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "OTH",
                    role: UttRole::CaBottom,
                    start_ms: s1,
                    end_ms: e1,
                    words: words1,
                });
                next_utt_idx += 1;
                cursor_ms = s0 + 2 * UTT_DURATION_MS;
                k += 2;
            }
            (OverlapConvention::LazyPrecedes, true, true) => {
                // Same temporal geometry as CA bracket; CHAT emission differs.
                let (s0, e0, s1, e1) = pair_overlap_ranges(cursor_ms);
                let words0 = mint_words(seed, WORDS_PER_UTT);
                let words1 = mint_words(seed ^ 0xcafe_babe, WORDS_PER_UTT);
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "MAI",
                    role: UttRole::LazyLeading,
                    start_ms: s0,
                    end_ms: e0,
                    words: words0,
                });
                next_utt_idx += 1;
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "OTH",
                    role: UttRole::LazyContinuation,
                    start_ms: s1,
                    end_ms: e1,
                    words: words1,
                });
                next_utt_idx += 1;
                cursor_ms = s0 + 2 * UTT_DURATION_MS;
                k += 2;
            }
            (OverlapConvention::InlineBackchannel, true, _) => {
                // Solo main utt with a short backchannel nested INSIDE it.
                let (s0, e0, bc_s, bc_e) = backchannel_ranges(cursor_ms);
                let words0 = mint_words(seed, WORDS_PER_UTT);
                let bc_words = mint_words(seed ^ 0xfeed_face, BACKCHANNEL_WORDS);
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "MAI",
                    role: UttRole::Solo,
                    start_ms: s0,
                    end_ms: e0,
                    words: words0,
                });
                next_utt_idx += 1;
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "OTH",
                    role: UttRole::Backchannel,
                    start_ms: bc_s,
                    end_ms: bc_e,
                    words: bc_words,
                });
                next_utt_idx += 1;
                cursor_ms = e0;
                // InlineBackchannel consumes only 1 main-timeline slot per
                // pair (the backchannel nests inside). Advance k by 2 so the
                // pair still counts against `n_utts`.
                k += 2;
            }
            _ => {
                // Solo / no-overlap / pair not possible: one main utt.
                let s0 = cursor_ms;
                let e0 = s0 + UTT_DURATION_MS;
                let words0 = mint_words(seed, WORDS_PER_UTT);
                layouts.push(UttLayout {
                    utt_index: next_utt_idx,
                    speaker: "MAI",
                    role: UttRole::Solo,
                    start_ms: s0,
                    end_ms: e0,
                    words: words0,
                });
                next_utt_idx += 1;
                cursor_ms = e0;
                k += 1;
            }
        }
    }

    layouts
}

/// Step 2: place each word uniformly within its utterance's time range and
/// flatten into one document-wide Vec of timed tokens.
#[derive(Debug, Clone)]
struct TimedWord {
    text: String,
    start_ms: u64,
    end_ms: u64,
    /// Stable tie-breaker for the temporal sort: preserves layout emission
    /// order when two words have identical start_ms.
    insertion_order: usize,
}

fn place_words_in_time(layouts: &[UttLayout]) -> Vec<TimedWord> {
    let mut out: Vec<TimedWord> = Vec::new();
    let mut insertion: usize = 0;
    for utt in layouts {
        let span = utt.end_ms.saturating_sub(utt.start_ms).max(1);
        let step = span / (utt.words.len().max(1) as u64);
        for (i, w) in utt.words.iter().enumerate() {
            let s = utt.start_ms + (i as u64) * step;
            let e = if i + 1 == utt.words.len() {
                utt.end_ms
            } else {
                s + step
            };
            out.push(TimedWord {
                text: w.clone(),
                start_ms: s,
                end_ms: e,
                insertion_order: insertion,
            });
            insertion += 1;
        }
    }
    out
}

/// Step 3: produce the ASR stream in TEMPORAL order by sorting across all
/// utterances. This is the key step that breaks document-order DP: under
/// overlap, utts A and B produce interleaved ASR tokens (A1, B1, A2, B2, ...)
/// even though CHAT lists all of A's words before any of B's.
fn interleave_asr(mut words: Vec<TimedWord>) -> Vec<TimedWord> {
    words.sort_by(|a, b| {
        a.start_ms
            .cmp(&b.start_ms)
            .then_with(|| a.end_ms.cmp(&b.end_ms))
            .then_with(|| a.insertion_order.cmp(&b.insertion_order))
    });
    words
}

/// Step 4: drop tokens with probability `rate`, deterministic on a stable
/// per-word seed (insertion_order). We cannot use array position because the
/// temporal re-sort has already happened — we need the drop mask to be
/// independent of sort order so the same word is always dropped across runs.
fn apply_asr_drop(words: Vec<TimedWord>, rate: f64) -> Vec<AsrTimingToken> {
    words
        .into_iter()
        .filter_map(|w| {
            let roll = (w.insertion_order as u64).wrapping_mul(LCG_MULT_GLIBC) ^ LCG_INC_GLIBC;
            let normalized = (roll % 1000) as f64 / 1000.0;
            if normalized < rate {
                None
            } else {
                Some(AsrTimingToken {
                    text: w.text,
                    start_ms: w.start_ms,
                    end_ms: w.end_ms,
                })
            }
        })
        .collect()
}

/// Step 5: emit the CHAT document in DOCUMENT order (layouts' order). Overlap
/// markup appears exactly where it belongs but the document never reveals the
/// temporal interleaving — which is what makes GlobalUtr's document-order DP
/// mis-align against the temporally re-sorted ASR.
fn emit_chat_source(layouts: &[UttLayout]) -> String {
    let mut chat = String::new();
    chat.push_str("@UTF8\n@Begin\n");
    chat.push_str("@Languages:\teng\n");
    chat.push_str("@Participants:\tMAI Main, OTH Other\n");
    chat.push_str("@ID:\teng|test|MAI||||||||\n");
    chat.push_str("@ID:\teng|test|OTH||||||||\n");

    for utt in layouts {
        let words_joined = utt.words.join(" ");
        let line = match utt.role {
            UttRole::CaTop => format!(
                "*{}:\t⌈ {} ⌉ {} .\n",
                utt.speaker,
                utt.words[0],
                utt.words[1..].join(" "),
            ),
            UttRole::CaBottom => format!(
                "*{}:\t⌊ {} ⌋ {} .\n",
                utt.speaker,
                utt.words[0],
                utt.words[1..].join(" "),
            ),
            UttRole::LazyContinuation => {
                format!("*{}:\t+< {} .\n", utt.speaker, words_joined)
            }
            UttRole::Solo | UttRole::LazyLeading | UttRole::Backchannel => {
                format!("*{}:\t{} .\n", utt.speaker, words_joined)
            }
        };
        chat.push_str(&line);
    }

    chat.push_str("@End\n");
    chat
}

/// Build a CHAT document + corresponding ASR token stream for a scenario.
///
/// # Algorithm
///
/// 1. [`layout_utterances_in_time`]: assign each utterance a `(start_ms,
///    end_ms)` range and a `UttRole`. Overlap-bearing pair conventions
///    (`CaBracket`, `LazyPrecedes`) produce two adjacent utts with
///    intersecting ranges. `InlineBackchannel` places short backchannel utts
///    nested inside a host's range.
/// 2. [`place_words_in_time`]: distribute each utt's words uniformly across
///    its range, producing a flat Vec of timed tokens tagged with an
///    insertion_order for stable sort / drop-mask seeding.
/// 3. [`interleave_asr`]: sort the tokens by `start_ms` to produce the
///    temporal-order ASR stream. Overlapping utts now have interleaved words
///    in the ASR stream by construction.
/// 4. [`apply_asr_drop`]: probabilistically drop tokens at `asr_missing_rate`.
/// 5. [`emit_chat_source`]: serialize the CHAT in DOCUMENT order (layouts'
///    order), with convention-appropriate markup.
///
/// Drift arises from the impedance mismatch between document-order CHAT and
/// temporal-order ASR. A monotone DP walking CHAT order over the temporally
/// re-sorted ASR cannot reconcile the two when words interleave — that is the
/// actual drift mechanism observed in MICASE-class files.
fn build_scenario(params: DriftParams) -> (ChatFile, Vec<AsrTimingToken>, Vec<ExpectedWindow>) {
    let layouts = layout_utterances_in_time(params);
    let timed = place_words_in_time(&layouts);
    let temporal = interleave_asr(timed);
    let asr_tokens = apply_asr_drop(temporal, params.asr_missing_rate);

    let expected_windows: Vec<ExpectedWindow> = layouts
        .iter()
        .map(|u| ExpectedWindow {
            utt_index: u.utt_index,
            expected_start_ms: u.start_ms,
            expected_end_ms: u.end_ms,
        })
        .collect();

    let chat_text = emit_chat_source(&layouts);
    // build_scenario is a test-fixture builder; tree-sitter grammar
    // is compiled into the binary so construction is infallible.
    #[allow(clippy::expect_used)]
    let parser = TreeSitterParser::new().expect("construct TreeSitterParser");
    let (chat_file, errors) = parse_lenient(&parser, &chat_text);
    assert!(
        errors.is_empty(),
        "parse_lenient rejected synthesized CHAT (first 400 chars shown): {:?}\n---\n{}",
        errors.iter().take(3).collect::<Vec<_>>(),
        &chat_text[..chat_text.len().min(400)],
    );
    (chat_file, asr_tokens, expected_windows)
}

/// Run UTR against the synthesized inputs and collect invariant violations.
///
/// `expected` is the synthetic ground-truth audio window per utterance — the
/// audio region that utterance's words actually occupy in the synthesized
/// token stream. When `None`, ground-truth drift detection is skipped (used
/// by helper self-tests that construct bullets by hand).
fn run_and_check(
    mut chat: ChatFile,
    tokens: &[AsrTimingToken],
    expected: Option<&[ExpectedWindow]>,
) -> (UtrResult, Vec<String>) {
    let result = inject_utr_timing(&mut chat, tokens);
    let mut violations: Vec<String> = Vec::new();
    check_bullet_integrity(&chat, &mut violations);
    check_utterance_monotonicity(&chat, &mut violations);
    if let Some(exp) = expected {
        check_bullet_within_ground_truth(&chat, exp, &mut violations);
    }
    (result, violations)
}

/// Invariant 3: every assigned bullet must land inside (or touching) the
/// utterance's ground-truth audio window, expanded by one `ms_per_word`
/// (500 ms) of slack on each side.
///
/// The slack tolerates the "utterance straddles a word boundary in the ASR
/// stream" case without flagging it as drift. What this invariant CATCHES is
/// the real drift pattern: a bullet whose start is thousands of milliseconds
/// earlier or later than the audio region where this utterance's words
/// actually live — i.e. the DP matched this utterance's words to tokens
/// belonging to a different utterance.
fn check_bullet_within_ground_truth(
    chat: &ChatFile,
    expected: &[ExpectedWindow],
    violations: &mut Vec<String>,
) {
    const SLACK_MS: u64 = 500; // one word of cadence slack per side
    let mut utt_ordinal = 0usize;
    for line in chat.lines.iter() {
        let Line::Utterance(utt) = line else { continue };
        let ordinal = utt_ordinal;
        utt_ordinal += 1;
        let Some(bullet) = utt.main.content.bullet.as_ref() else {
            continue;
        };
        let Some(exp) = expected.get(ordinal) else {
            continue;
        };
        // `exp.utt_index == ordinal` by construction of `expected_windows` in
        // `build_scenario` (it's just `layouts.iter().map(...).collect()` in the
        // same order). No runtime check needed — the index correspondence is
        // structural.
        let lo = exp.expected_start_ms.saturating_sub(SLACK_MS);
        let hi = exp.expected_end_ms.saturating_add(SLACK_MS);
        let s = bullet.timing.start_ms;
        let e = bullet.timing.end_ms;
        if s < lo || e > hi {
            violations.push(format!(
                "utt #{ordinal}: bullet [{s},{e}]ms outside ground-truth window \
                 [{lo},{hi}]ms (expected [{exp_s},{exp_e}]ms ± {SLACK_MS}ms slack)",
                exp_s = exp.expected_start_ms,
                exp_e = exp.expected_end_ms,
            ));
        }
    }
    debug_assert!(
        utt_ordinal == expected.len(),
        "utt_ordinal/expected_windows desync: {} vs {}",
        utt_ordinal,
        expected.len(),
    );
}

/// Invariant 1: every bullet has `end_ms > start_ms`.
///
/// `run_global_utr` already refuses to emit a bullet with `start >= end`
/// (zero-duration frames go to `unmatched`), so this serves as a belt-and-
/// braces check — if a future strategy regresses that rule, this fires.
fn check_bullet_integrity(chat: &ChatFile, violations: &mut Vec<String>) {
    for (i, line) in chat.lines.iter().enumerate() {
        let Line::Utterance(utt) = line else { continue };
        let Some(bullet) = utt.main.content.bullet.as_ref() else {
            continue;
        };
        let (s, e) = (bullet.timing.start_ms, bullet.timing.end_ms);
        if e <= s {
            violations.push(format!(
                "line {i}: non-positive bullet duration (start={s}ms, end={e}ms)"
            ));
        }
    }
}

/// Invariant 2: adjacent non-overlap utterance bullets are strictly monotone
/// in `start_ms`.
///
/// Overlap-continuation utterances (`+<` linker OR `⌊`-bearing text) are
/// excluded from BOTH sides of the comparison, matching the Task 1.1
/// regression-harness arm `UtteranceBulletMonotonicityPreserved`.
fn check_utterance_monotonicity(chat: &ChatFile, violations: &mut Vec<String>) {
    let mut prev: Option<MonotoneProbe> = None;
    let mut utt_ordinal: usize = 0;
    for (line_idx, line) in chat.lines.iter().enumerate() {
        let Line::Utterance(utt) = line else { continue };
        let this_ordinal = utt_ordinal;
        utt_ordinal += 1;
        let Some(bullet) = utt.main.content.bullet.as_ref() else {
            continue;
        };
        let is_overlap = utt
            .main
            .content
            .linkers
            .0
            .contains(&Linker::LazyOverlapPrecedes)
            || overlap_markers::extract_overlap_info(&utt.main.content.content.0)
                .has_bottom_overlap();
        if is_overlap {
            continue;
        }
        let this_start = bullet.timing.start_ms;
        if let Some(p) = prev
            && this_start <= p.start_ms
        {
            violations.push(format!(
                "line {line_idx} (utt #{this_ordinal}): non-monotonic start \
                 (this={this_start}ms <= prev={prev_ms}ms from utt #{prev_idx})",
                prev_ms = p.start_ms,
                prev_idx = p.utt_index,
            ));
        }
        prev = Some(MonotoneProbe {
            utt_index: this_ordinal,
            start_ms: this_start,
        });
    }
    debug_assert!(
        utt_ordinal
            == chat
                .lines
                .iter()
                .filter(|l| matches!(l, Line::Utterance(_)))
                .count(),
        "utt_ordinal desync with chat.lines utterance count: {}",
        utt_ordinal,
    );
}

// --------------------------------------------------------------------------
// Helper self-tests.
//
// Confirm `check_bullet_integrity` and `check_utterance_monotonicity` detect
// the shapes they claim to, on tiny hand-crafted documents. If these pass,
// the scenario assertions can be trusted.
// --------------------------------------------------------------------------

#[cfg(test)]
mod helper_self_tests {
    use super::*;
    use talkbank_model::model::Bullet;

    /// Build a 3-utterance CHAT with the supplied bullet timings (None => untimed).
    /// Optionally mark utterance `i` as lazy-overlap (`+<`).
    fn make_three_utt_chat(
        bullets: [Option<(u64, u64)>; 3],
        lazy_overlap_mask: [bool; 3],
    ) -> ChatFile {
        let parser = TreeSitterParser::new().unwrap();
        let mut text = String::new();
        text.push_str("@UTF8\n@Begin\n");
        text.push_str("@Languages:\teng\n");
        text.push_str("@Participants:\tMAI Main, OTH Other\n");
        text.push_str("@ID:\teng|test|MAI||||||||\n");
        text.push_str("@ID:\teng|test|OTH||||||||\n");
        for (i, &is_lazy) in lazy_overlap_mask.iter().enumerate() {
            let prefix = if is_lazy { "+< " } else { "" };
            text.push_str(&format!("*MAI:\t{prefix}hello world number{i} .\n"));
        }
        text.push_str("@End\n");
        let (mut chat, errs) = parse_lenient(&parser, &text);
        assert!(
            errs.is_empty(),
            "helper self-test CHAT parse errors: {errs:?}"
        );
        let mut utt_i = 0;
        for line in chat.lines.iter_mut() {
            if let Line::Utterance(utt) = line {
                if let Some((s, e)) = bullets[utt_i] {
                    utt.main.content.bullet = Some(Bullet::new(s, e));
                }
                utt_i += 1;
                if utt_i == 3 {
                    break;
                }
            }
        }
        chat
    }

    #[test]
    fn bullet_integrity_detects_zero_duration() {
        let chat = make_three_utt_chat([Some((1000, 1000)), Some((2000, 3000)), None], [false; 3]);
        let mut v = Vec::new();
        check_bullet_integrity(&chat, &mut v);
        assert_eq!(v.len(), 1, "expected one violation, got: {v:?}");
        assert!(v[0].contains("non-positive"), "msg: {}", v[0]);
    }

    #[test]
    fn bullet_integrity_passes_clean_document() {
        let chat = make_three_utt_chat(
            [Some((0, 500)), Some((500, 1000)), Some((1000, 1500))],
            [false; 3],
        );
        let mut v = Vec::new();
        check_bullet_integrity(&chat, &mut v);
        assert!(v.is_empty(), "unexpected violations: {v:?}");
    }

    #[test]
    fn monotonicity_detects_backwards_start() {
        // Utt 1 starts BEFORE utt 0 → should flag.
        let chat = make_three_utt_chat(
            [Some((1000, 1500)), Some((500, 900)), Some((2000, 2500))],
            [false; 3],
        );
        let mut v = Vec::new();
        check_utterance_monotonicity(&chat, &mut v);
        assert_eq!(v.len(), 1, "expected one violation, got: {v:?}");
        assert!(v[0].contains("non-monotonic"), "msg: {}", v[0]);
    }

    #[test]
    fn monotonicity_skips_lazy_overlap_utterance() {
        // Utt 1 has +< AND starts before utt 0 — legitimate overlap, should NOT flag.
        // Utt 2 must still be ahead of utt 0 (the last non-overlap anchor).
        let chat = make_three_utt_chat(
            [Some((1000, 1500)), Some((500, 900)), Some((2000, 2500))],
            [false, true, false],
        );
        let mut v = Vec::new();
        check_utterance_monotonicity(&chat, &mut v);
        assert!(v.is_empty(), "unexpected violations: {v:?}");
    }

    #[test]
    fn monotonicity_passes_strictly_monotone_document() {
        let chat = make_three_utt_chat(
            [Some((0, 500)), Some((500, 1000)), Some((1000, 1500))],
            [false; 3],
        );
        let mut v = Vec::new();
        check_utterance_monotonicity(&chat, &mut v);
        assert!(v.is_empty(), "unexpected violations: {v:?}");
    }
}

// --------------------------------------------------------------------------
// Scenario tests
//
// Each reports RED / GREEN status of the CURRENT `inject_utr_timing` dispatch
// (GlobalUtr for non-overlap, TwoPassOverlapUtr when overlap markers are
// detected). Phase 3 will flip the dispatch to SegmentAwareUtr without
// touching this file.
// --------------------------------------------------------------------------

/// Sanity: a short file with CA brackets should align cleanly on the current
/// pipeline. This is the control: if this ever goes RED, the problem is with
/// the scenario builder, not the production code.
#[test]
fn short_file_baseline() {
    let (chat, tokens, expected) = build_scenario(DriftParams {
        n_utts: 30,
        convention: OverlapConvention::CaBracket,
        overlap_density: 0.2,
        asr_missing_rate: 0.0,
        stopword_only: false,
    });
    let (result, violations) = run_and_check(chat, &tokens, Some(&expected));
    assert!(
        violations.is_empty(),
        "short_file_baseline should have zero invariant violations on current pipeline\n\
         result={result:?}\nviolations:\n  {}",
        violations.join("\n  "),
    );
}

/// Long-file CA-bracket drift — mimics MICASE / samtale. Dense `⌈⌉/⌊⌋` pairs
/// plus ~10% missing ASR tokens force the monotonic DP to commit to the wrong
/// repeated-word match and accumulate offset over hundreds of utterances.
///
/// Expected RED on GlobalUtr / TwoPassOverlapUtr (the current dispatch).
/// Expected GREEN after Phase 3 flips to SegmentAwareUtr.
///
/// `#[ignore]` because the test is currently RED on the default pipeline —
/// this is by design, as the test codifies a bug we have not yet fixed.
/// Run explicitly with `cargo test … -- --ignored` to see the RED evidence.
#[test]
#[ignore = "RED on GlobalUtr by design; evidence of drift bug, not a fix-regression"]
fn drift_ca_overlap_long_file() {
    // Retuned for temporal-interleaving model (2026-04-22): higher
    // asr_missing_rate forces the DP off the correct path under
    // CA-bracket-induced interleaving.
    let (chat, tokens, expected) = build_scenario(DriftParams {
        n_utts: 500,
        convention: OverlapConvention::CaBracket,
        overlap_density: 0.60,
        asr_missing_rate: 0.25,
        stopword_only: false,
    });
    let (result, violations) = run_and_check(chat, &tokens, Some(&expected));
    assert!(
        violations.is_empty(),
        "drift_ca_overlap_long_file: {} violations (showing up to 10)\n\
         result={result:?}\n  {}",
        violations.len(),
        violations
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// Long-file `+<` drift — mimics the biling class. Reordered transcript
/// against temporally-ordered ASR is hard for a single monotonic DP.
#[test]
#[ignore = "RED on GlobalUtr by design; evidence of drift bug, not a fix-regression"]
fn drift_lazy_overlap_long_file() {
    // Retuned for temporal-interleaving model (2026-04-22).
    let (chat, tokens, expected) = build_scenario(DriftParams {
        n_utts: 500,
        convention: OverlapConvention::LazyPrecedes,
        overlap_density: 0.45,
        asr_missing_rate: 0.15,
        stopword_only: false,
    });
    let (result, violations) = run_and_check(chat, &tokens, Some(&expected));
    assert!(
        violations.is_empty(),
        "drift_lazy_overlap_long_file: {} violations (showing up to 10)\n\
         result={result:?}\n  {}",
        violations.len(),
        violations
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// Long-file `&*SPK:` inline-backchannel drift — mimics rhd. ASR does not
/// emit a token for the interjected backchannel, so the DP aligns the
/// main-speaker's post-backchannel words one token too early, drifting
/// subsequent utterances.
#[test]
#[ignore = "RED on GlobalUtr by design; evidence of drift bug, not a fix-regression"]
fn drift_inline_backchannel_long_file() {
    // Retuned for temporal-interleaving model (2026-04-22): nested backchannel
    // utts produce temporal interleave + frequent "unmatched 2-word" failures.
    let (chat, tokens, expected) = build_scenario(DriftParams {
        n_utts: 500,
        convention: OverlapConvention::InlineBackchannel,
        overlap_density: 0.45,
        asr_missing_rate: 0.10,
        stopword_only: false,
    });
    let (result, violations) = run_and_check(chat, &tokens, Some(&expected));
    assert!(
        violations.is_empty(),
        "drift_inline_backchannel_long_file: {} violations (showing up to 10)\n\
         result={result:?}\n  {}",
        violations.len(),
        violations
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// All three conventions interleaved — mimics real MICASE files, which mix
/// CA brackets, `+<`, and inline backchannel tokens freely.
#[test]
#[ignore = "RED on GlobalUtr by design; evidence of drift bug, not a fix-regression"]
fn drift_mixed_conventions() {
    // Retuned for temporal-interleaving model (2026-04-22).
    let (chat, tokens, expected) = build_scenario(DriftParams {
        n_utts: 500,
        convention: OverlapConvention::Mixed,
        overlap_density: 0.60,
        asr_missing_rate: 0.15,
        stopword_only: false,
    });
    let (result, violations) = run_and_check(chat, &tokens, Some(&expected));
    assert!(
        violations.is_empty(),
        "drift_mixed_conventions: {} violations (showing up to 10)\n\
         result={result:?}\n  {}",
        violations.len(),
        violations
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// Diagnostic-only sweep that verifies drift scenarios fail robustly across a
/// small parameter grid. Run manually with
/// `cargo test -p batchalign-chat-ops --lib drift_margin_sweep_diagnostic -- --ignored --nocapture`.
///
/// Purpose: prove the drift RED-gate isn't knife-edge. If all grid cells
/// produce >= 20 violations on GlobalUtr, the gate is robust. If some produce
/// 1-2 violations, retune constants.
#[test]
#[ignore]
fn drift_margin_sweep_diagnostic() {
    use std::fmt::Write;
    // Widened (2026-04-22) to cover the production drift-test params, which
    // use density up to 0.60 and missing up to 0.25. This lets the diagnostic
    // demonstrate that those production points are not knife-edge.
    let densities = [0.25, 0.35, 0.45, 0.60];
    let missing_rates = [0.05, 0.10, 0.15, 0.20, 0.25];
    let mut report = String::new();
    writeln!(
        &mut report,
        "convention        density  missing  violations"
    )
    .unwrap();
    for convention in [
        OverlapConvention::CaBracket,
        OverlapConvention::LazyPrecedes,
        OverlapConvention::InlineBackchannel,
    ] {
        for &d in &densities {
            for &r in &missing_rates {
                let params = DriftParams {
                    n_utts: 500,
                    convention,
                    overlap_density: d,
                    asr_missing_rate: r,
                    stopword_only: false,
                };
                let (chat, tokens, expected) = build_scenario(params);
                let (_result, violations) = run_and_check(chat, &tokens, Some(&expected));
                writeln!(
                    &mut report,
                    "{:<18} {:>6.2}  {:>6.2}  {:>10}",
                    format!("{:?}", convention),
                    d,
                    r,
                    violations.len(),
                )
                .unwrap();
            }
        }
    }
    // Print to stdout so `--nocapture` shows it:
    println!("\n{report}");
}

/// Anchor-sparse transcript — no overlap markers, but every word is a
/// high-frequency stopword (< 4 chars). Tests the Option-4 fallback for
/// anchor-starved DP: with no unambiguous rare-word anchors, the global DP
/// has little signal to lock onto. Utterances UTR cannot place should land
/// in `UtrResult.unmatched`, not receive a wrong bullet.
#[test]
fn anchor_sparse_stopword_heavy() {
    let (chat, tokens, expected) = build_scenario(DriftParams {
        n_utts: 500,
        convention: OverlapConvention::None,
        overlap_density: 0.0,
        asr_missing_rate: 0.15,
        stopword_only: true,
    });
    let (result, violations) = run_and_check(chat, &tokens, Some(&expected));
    assert!(
        violations.is_empty(),
        "anchor_sparse_stopword_heavy: {} violations (showing up to 10)\n\
         result={result:?}\n  {}",
        violations.len(),
        violations
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}
