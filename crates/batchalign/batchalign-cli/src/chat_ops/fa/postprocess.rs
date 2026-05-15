//! Post-processing: fix end times, bound by utterance, update bullets.

use talkbank_model::alignment::helpers::{WordItem, WordItemMut, walk_words, walk_words_mut};
use talkbank_model::model::{
    Bullet, BulletSource, Utterance, UtteranceContent, Word, WordCategory,
};

use super::get_word_timing;
use super::{FaTimingMode, TimeSpan};

/// Maximum internal gap (ms) that Continuous mode may collapse into the
/// previous word.
///
/// Small word-to-word gaps are a useful smoothing target, but multi-second
/// silences or mistracked spans should remain visible instead of turning one
/// word into a dominant 10-second token.
const MAX_CONTINUOUS_INTERNAL_GAP_MS: u64 = 1_000;
const MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS: u64 = 40;
const MAX_CONTINUOUS_WORD_PROPORTION_NUMERATOR: u64 = 2;
const MAX_CONTINUOUS_WORD_PROPORTION_DENOMINATOR: u64 = 5;
/// When a rerun already has `%wor`, authoritative-bullet clamping normally
/// keeps FA word timings inside the previous utterance window. However, some
/// stale rerun bullets truncate the final word to a near-zero tail. Allow the
/// last timed word to heal a small overrun instead of preserving the collapse.
const MIN_HEALED_FINAL_WORD_DURATION_MS: u64 = 100;
const MAX_HEALED_FINAL_WORD_OVERRUN_MS: u64 = 500;

/// Post-process timings: set word end times, bound by utterance, update bullets.
///
/// 1. For `Continuous` mode: set each word's end time to the next word's start
///    time only when the internal gap is plausibly small
/// 2. Bound all word times within utterance bullet range
/// 3. Drop invalid timings (start >= end)
/// 4. Update utterance bullet from word timings
///
/// Returns the number of words whose timing was dropped due to clamping.
pub fn postprocess_utterance_timings(
    utterance: &mut Utterance,
    timing_mode: FaTimingMode,
) -> usize {
    let mut word_timings: Vec<Option<TimeSpan>> = Vec::new();
    collect_word_timings(&utterance.main.content.content, &mut word_timings);
    let mut word_is_compound_filler: Vec<bool> = Vec::new();
    collect_compound_filler_flags(
        &utterance.main.content.content,
        &mut word_is_compound_filler,
    );
    let mut word_is_filler: Vec<bool> = Vec::new();
    collect_filler_flags(&utterance.main.content.content, &mut word_is_filler);

    if word_timings.is_empty() {
        return 0;
    }
    debug_assert_eq!(word_timings.len(), word_is_compound_filler.len());
    debug_assert_eq!(word_timings.len(), word_is_filler.len());
    let mut words_dropped = 0;
    let utterance_span_ms = utterance
        .main
        .content
        .bullet
        .as_ref()
        .map(|bullet| bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms))
        .or_else(|| {
            let first = word_timings
                .iter()
                .find_map(|timing| timing.map(|span| span.start_ms))?;
            let last = word_timings
                .iter()
                .rev()
                .find_map(|timing| timing.map(|span| span.end_ms))?;
            Some(last.saturating_sub(first))
        });

    // For Continuous mode: set each word's end_ms to next word's start_ms.
    // Uses a backward pass (O(w)) instead of per-word forward scan (O(w²)).
    if timing_mode == FaTimingMode::Continuous {
        let n = word_timings.len();

        // Last timed word: extend to utterance bullet end or +500ms
        // (must happen first — the backward pass below would leave it unchanged
        // since there's no next_start, but we need to set its end explicitly)
        for i in (0..n).rev() {
            if let Some(span) = word_timings[i] {
                if span.start_ms == span.end_ms {
                    let fallback_end = if let Some(ref bullet) = utterance.main.content.bullet {
                        let utt_end = bullet.timing.end_ms;
                        if utt_end > span.start_ms {
                            utt_end
                        } else {
                            span.start_ms + 500
                        }
                    } else {
                        span.start_ms + 500
                    };
                    word_timings[i] = Some(TimeSpan::new(span.start_ms, fallback_end));
                }
                break;
            }
        }

        // Backward pass: track the next timed word's start_ms and propagate it
        // as the current word's end_ms only for plausibly small internal gaps.
        let mut next_start: Option<u64> = None;
        let mut next_is_filler = false;
        for i in (0..n).rev() {
            if let Some(span) = word_timings[i] {
                if let Some(ns) = next_start {
                    let gap_ms = ns.saturating_sub(span.end_ms);
                    let bridged_duration = ns.saturating_sub(span.start_ms);
                    let lexical_to_filler_bridge_is_plausible =
                        if next_is_filler && !word_is_filler[i] {
                            span.duration_ms() < MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS
                                && bridged_duration_stays_within_proportion_cap(
                                    utterance_span_ms,
                                    bridged_duration,
                                )
                        } else {
                            true
                        };
                    let filler_bridge_is_plausible = if word_is_filler[i] {
                        bridged_duration_stays_within_proportion_cap(
                            utterance_span_ms,
                            bridged_duration,
                        )
                    } else {
                        true
                    };
                    let should_fill_gap = if next_is_filler {
                        lexical_to_filler_bridge_is_plausible && filler_bridge_is_plausible
                    } else {
                        gap_ms <= MAX_CONTINUOUS_INTERNAL_GAP_MS && filler_bridge_is_plausible
                    };
                    if should_fill_gap && !word_is_compound_filler[i] {
                        word_timings[i] = Some(TimeSpan::new(span.start_ms, ns));
                    } else if span.start_ms == span.end_ms {
                        let capped_end = (span.start_ms + 500).min(ns);
                        word_timings[i] = Some(TimeSpan::new(span.start_ms, capped_end));
                    }
                }
                next_start = Some(span.start_ms);
                next_is_filler = word_is_filler[i];
            }
        }

        rebalance_near_zero_lexical_words_from_following_spans(&mut word_timings, &word_is_filler);
        rebalance_near_zero_lexical_words_from_preceding_spans(&mut word_timings, &word_is_filler);
    }

    // Bound by utterance bullet range — but ONLY when both conditions hold:
    //
    // 1. The bullet is `BulletSource::Authoritative` (not a runtime UTR hint).
    //    UTR-hinted bullets (`BulletSource::Utr`) are provisional estimates from
    //    ASR token timestamps.  They can be much narrower than the actual speech
    //    window: e.g., Rev.AI may produce a 220ms hint for a 3-second utterance
    //    when it only recognised the first word.  Clamping FA word timings to a
    //    UTR hint would drop every word beyond the first.
    //
    // 2. The utterance already has a `%wor` tier (i.e., this is a RE-alignment,
    //    not a first-time alignment).
    //    After `transcribe` + `utseg`, utterance bullets are ASR-derived (from
    //    UTR token matching) and are serialized as `Authoritative` (BulletSource
    //    is not persisted in CHAT text).  These bullets can be as narrow as the
    //    ASR-matched span for one word (e.g., 220ms for a 3-second sentence when
    //    Rev.AI only matched the first word).  FA, given a wider group audio
    //    window, correctly aligns all words — but clamping to the narrow bullet
    //    would drop all but the first, breaking the output.
    //    The `%wor` tier is only present after a previous FA run, which means the
    //    utterance bullet was established by FA from word timings and is wide
    //    enough to cover the speech.  That is the only case where clamping is safe.
    //
    // Self-healing: `update_utterance_bullet` overwrites UTR hints with the FA
    // word span after postprocess.  Clamping to a narrow UTR/ASR bullet before
    // that overwrite would prevent the self-healing from ever running.
    let has_fa_wor = utterance.wor_tier().is_some();
    if let Some(ref bullet) = utterance.main.content.bullet
        && bullet.source == BulletSource::Authoritative
        && has_fa_wor
    {
        let utt_start = bullet.timing.start_ms;
        let utt_end = bullet.timing.end_ms;
        let last_timed_idx = word_timings.iter().rposition(|timing| timing.is_some());

        for (idx, timing) in word_timings.iter_mut().enumerate() {
            if let Some(span) = timing {
                let clamped_start = span.start_ms.max(utt_start);
                let mut clamped_end = span.end_ms.min(utt_end);
                if Some(idx) == last_timed_idx
                    && clamped_end < span.end_ms
                    && clamped_end.saturating_sub(clamped_start) < MIN_HEALED_FINAL_WORD_DURATION_MS
                {
                    let overrun_ms = span.end_ms.saturating_sub(utt_end);
                    if overrun_ms <= MAX_HEALED_FINAL_WORD_OVERRUN_MS {
                        clamped_end = span.end_ms;
                    }
                }
                if clamped_start >= clamped_end {
                    tracing::warn!(
                        "word timing dropped: clamped to utterance boundary made start >= end"
                    );
                    words_dropped += 1;
                    *timing = None;
                } else {
                    *span = TimeSpan::new(clamped_start, clamped_end);
                }
            }
        }
    }

    // Write timings back to the AST
    let mut idx = 0;
    set_word_timings(&mut utterance.main.content.content, &word_timings, &mut idx);

    words_dropped
}

fn rebalance_near_zero_lexical_words_from_following_spans(
    word_timings: &mut [Option<TimeSpan>],
    word_is_filler: &[bool],
) {
    debug_assert_eq!(word_timings.len(), word_is_filler.len());

    for i in 0..word_timings.len().saturating_sub(1) {
        let (Some(current), Some(next)) = (word_timings[i], word_timings[i + 1]) else {
            continue;
        };
        if word_is_filler[i] {
            continue;
        }
        if current.duration_ms() >= MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS {
            continue;
        }
        if current.end_ms != next.start_ms {
            continue;
        }

        let needed_ms = MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS - current.duration_ms();
        if next.duration_ms() < needed_ms + MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS {
            continue;
        }

        let new_boundary = next.start_ms + needed_ms;
        word_timings[i] = Some(TimeSpan::new(current.start_ms, new_boundary));
        word_timings[i + 1] = Some(TimeSpan::new(new_boundary, next.end_ms));
    }
}

fn rebalance_near_zero_lexical_words_from_preceding_spans(
    word_timings: &mut [Option<TimeSpan>],
    word_is_filler: &[bool],
) {
    debug_assert_eq!(word_timings.len(), word_is_filler.len());

    for i in 1..word_timings.len() {
        let (Some(previous), Some(current)) = (word_timings[i - 1], word_timings[i]) else {
            continue;
        };
        if word_is_filler[i] {
            continue;
        }
        if current.duration_ms() >= MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS {
            continue;
        }
        if previous.end_ms != current.start_ms {
            continue;
        }

        let needed_ms = MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS - current.duration_ms();
        if previous.duration_ms() < needed_ms + MIN_CONTINUOUS_HEALABLE_WORD_DURATION_MS {
            continue;
        }

        let new_boundary = current.start_ms - needed_ms;
        word_timings[i - 1] = Some(TimeSpan::new(previous.start_ms, new_boundary));
        word_timings[i] = Some(TimeSpan::new(new_boundary, current.end_ms));
    }
}

fn bridged_duration_stays_within_proportion_cap(
    utterance_span_ms: Option<u64>,
    bridged_duration_ms: u64,
) -> bool {
    utterance_span_ms.is_some_and(|utterance_span_ms| {
        bridged_duration_ms.saturating_mul(MAX_CONTINUOUS_WORD_PROPORTION_DENOMINATOR)
            <= utterance_span_ms.saturating_mul(MAX_CONTINUOUS_WORD_PROPORTION_NUMERATOR)
    })
}

/// Collect current word timings in document order.
///
/// Visits ALL words (no alignability filter). For replaced words, only the
/// original (spoken) word's timing is collected.
pub(super) fn collect_word_timings(content: &[UtteranceContent], out: &mut Vec<Option<TimeSpan>>) {
    // domain=None: recurse into all groups unconditionally
    walk_words(content, None, &mut |leaf| match leaf {
        WordItem::Word(word) => {
            out.push(get_word_timing(word));
        }
        WordItem::ReplacedWord(replaced) => {
            out.push(get_word_timing(&replaced.word));
        }
        WordItem::Separator(_) => {}
    });
}

fn collect_compound_filler_flags(content: &[UtteranceContent], out: &mut Vec<bool>) {
    walk_words(content, None, &mut |leaf| match leaf {
        WordItem::Word(word) => {
            out.push(super::split_compound_filler(word).len() > 1);
        }
        WordItem::ReplacedWord(replaced) => {
            out.push(super::split_compound_filler(&replaced.word).len() > 1);
        }
        WordItem::Separator(_) => {}
    });
}

fn collect_filler_flags(content: &[UtteranceContent], out: &mut Vec<bool>) {
    walk_words(content, None, &mut |leaf| match leaf {
        WordItem::Word(word) => {
            out.push(word.category == Some(WordCategory::Filler));
        }
        WordItem::ReplacedWord(replaced) => {
            out.push(replaced.word.category == Some(WordCategory::Filler));
        }
        WordItem::Separator(_) => {}
    });
}

/// Write timings back into word AST nodes.
///
/// Visits ALL words in document order (same order as `collect_word_timings`).
/// For replaced words, sets timing on the original (spoken) word only.
fn set_word_timings(
    content: &mut [UtteranceContent],
    timings: &[Option<TimeSpan>],
    idx: &mut usize,
) {
    // domain=None: recurse into all groups unconditionally
    walk_words_mut(content, None, &mut |leaf| match leaf {
        WordItemMut::Word(word) => {
            set_word_timing(word, timings, idx);
        }
        WordItemMut::ReplacedWord(replaced) => {
            set_word_timing(&mut replaced.word, timings, idx);
        }
        WordItemMut::Separator(_) => {}
    });
}

fn set_word_timing(word: &mut Word, timings: &[Option<TimeSpan>], idx: &mut usize) {
    if *idx < timings.len() {
        match timings[*idx] {
            Some(span) => {
                word.inline_bullet = Some(Bullet::new(span.start_ms, span.end_ms));
            }
            None => {
                word.inline_bullet = None;
            }
        }
    }
    *idx += 1;
}
