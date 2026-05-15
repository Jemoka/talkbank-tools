//! Full-file orchestration: apply FA results, enforce monotonicity, enforce E704.

use std::collections::HashMap;

use talkbank_model::UtteranceIdx;
use talkbank_model::alignment::resolve_wor_timing_sidecar;
use talkbank_model::model::{
    BracketedItem, ChatFile, DependentTier, Line, Utterance, UtteranceContent,
};

use super::FaTimingMode;
use super::injection::inject_timings_for_utterance;
use super::postprocess::postprocess_utterance_timings;
use super::{
    FaGroup, WordTiming, add_wor_tier, count_alignable_main_words, get_utterance_mut,
    update_utterance_bullet,
};

/// Apply FA results to a ChatFile: inject timings, postprocess, optionally
/// generate %wor, enforce monotonicity, enforce E704 same-speaker non-overlap.
///
/// `groups` and `responses` must be parallel: `responses[i]` is the aligned
/// timings for `groups[i]`.
///
/// When `write_wor` is `true`, a `%wor` tier is generated for each utterance.
/// When `false`, existing `%wor` tiers are left untouched and no new ones are added.
pub fn apply_fa_results(
    chat_file: &mut ChatFile,
    groups: &[FaGroup],
    responses: &[Vec<Option<WordTiming>>],
    timing_mode: FaTimingMode,
    write_wor: bool,
) -> Vec<talkbank_transform::decisions::DecisionRecord> {
    let mut decisions = Vec::new();
    // 0. Strip stale decision tiers (%xalign / %xrev) from any previous FA run.
    //
    // This must happen unconditionally — not gated on whether new decisions will
    // be produced.  Without an unconditional strip, a clean re-run (no new
    // decisions) leaves the previous run's tiers in place; the NEXT run that
    // DOES produce decisions then appends to them, creating duplicates.
    talkbank_transform::decisions::strip_decision_tiers(chat_file);

    // 1. Strip InternalBullet tokens left over from parsing.
    //
    // When the FA pipeline receives CHAT text that already has UTR-injected
    // bullets, the parser creates InternalBullet content items. FA then
    // sets word inline_bullet + utterance .bullet via update_utterance_bullet.
    // Without stripping, both the old InternalBullet AND the new .bullet
    // are serialized, producing "two timestamps on the main line".
    for (group, _) in groups.iter().zip(responses.iter()) {
        for &utt_idx in &group.utterance_indices {
            if let Some(utt) = get_utterance_mut(chat_file, utt_idx) {
                strip_internal_bullet_tokens(&mut utt.main.content.content.0);
            }
        }
    }

    // 1. Distribute timings from each group's response to utterances
    for (group, timings) in groups.iter().zip(responses.iter()) {
        let mut timing_offset: usize = 0;

        for &utt_idx in &group.utterance_indices {
            let utt = match get_utterance_mut(chat_file, utt_idx) {
                Some(u) => u,
                None => continue,
            };
            inject_timings_for_utterance(utt, timings, &mut timing_offset);
        }
    }

    // 2. Postprocess all grouped utterances
    let all_utt_indices: Vec<UtteranceIdx> = groups
        .iter()
        .flat_map(|g| g.utterance_indices.iter().copied())
        .collect();

    for &utt_idx in &all_utt_indices {
        if let Some(utt) = get_utterance_mut(chat_file, utt_idx) {
            let words_dropped = postprocess_utterance_timings(utt, timing_mode);
            if words_dropped > 0 {
                decisions.push(talkbank_transform::decisions::DecisionRecord {
                    line_idx: utt_idx.0,
                    speaker: utt.main.speaker.as_str().to_string(),
                    strategy: talkbank_transform::decisions::DecisionStrategy::Fa(
                        talkbank_transform::decisions::FaStrategy::WordsTimingDropped,
                    ),
                    reason: format!("count={words_dropped} reason=clamped_to_utterance_boundary"),
                    needs_review: true,
                });
            }
            update_utterance_bullet(utt);
            if write_wor {
                add_wor_tier(utt);
            }
        }
    }

    // NOTE: E362 (monotonicity) and E704 (same-speaker overlap) enforcement
    // was removed here. These passes aggressively stripped timing from
    // utterances that had "imperfect but usable" timings, causing severe
    // regressions vs batchalign 0.8.x (up to 60% timing loss on real data).
    // The CHAT validator in talkbank-tools flags these violations after the
    // fact — the FA pipeline should not silently destroy timing data.

    decisions
}

/// Refresh a CHAT file that already carries reusable `%wor` timing.
///
/// This is the cheap rerun path for `align`. Instead of sending audio back
/// through the FA worker, the function:
///
/// 1. aligns each `%wor` tier back to the main tier,
/// 2. rehydrates main-tier `inline_bullet` timing from `%wor`,
/// 3. removes any parsed `InternalBullet` tokens left over from roundtripped
///    serialization,
/// 4. recomputes utterance bullets, and
/// 5. optionally regenerates `%wor`.
///
/// Callers should only use this after [`super::has_reusable_wor_timing`]
/// succeeds.
pub fn refresh_existing_alignment(chat_file: &mut ChatFile, write_wor: bool) {
    for line in &mut chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        if count_alignable_main_words(utterance) == 0 {
            continue;
        }

        let refreshed = refresh_existing_alignment_for_utterance(utterance, write_wor);
        if !refreshed {
            tracing::warn!(
                "skipping utterance with unreusable %wor timing in refresh_existing_alignment"
            );
        }
    }
}

/// Return `true` when one utterance has reusable `%wor` timing.
///
/// This is the per-utterance form of the cheap rerun check. It is useful for
/// selective reuse in incremental align workflows where only some utterances
/// remain trustworthy after manual edits.
pub fn has_reusable_wor_timing_for_utterance(utterance: &Utterance) -> bool {
    collect_wor_backed_timings(utterance).is_some()
}

/// Refresh one utterance from its existing `%wor` timing.
///
/// Returns `true` when the utterance had a clean reusable `%wor` mapping and
/// was refreshed successfully. Returns `false` when `%wor` was missing,
/// mismatched, or partially untimed.
pub fn refresh_existing_alignment_for_utterance(
    utterance: &mut Utterance,
    write_wor: bool,
) -> bool {
    let Some(timings) = collect_wor_backed_timings(utterance) else {
        return false;
    };

    strip_internal_bullet_tokens(&mut utterance.main.content.content.0);
    let mut offset = 0usize;
    inject_timings_for_utterance(utterance, &timings, &mut offset);
    update_utterance_bullet(utterance);
    if write_wor {
        add_wor_tier(utterance);
    }
    true
}

/// Refresh timing for utterances with reusable `%wor`, leaving stale ones
/// untouched for FA worker processing.
///
/// This is the per-utterance counterpart to [`refresh_existing_alignment()`].
/// Unlike that function (which asserts all utterances are reusable), this one
/// only refreshes utterances in the provided set, skipping stale ones that
/// will go through FA workers.
pub fn refresh_reusable_utterances(
    chat_file: &mut ChatFile,
    reusable_indices: &std::collections::HashSet<usize>,
    write_wor: bool,
) {
    let mut utt_idx = 0usize;
    for line in &mut chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        if reusable_indices.contains(&utt_idx) {
            let refreshed = refresh_existing_alignment_for_utterance(utterance, write_wor);
            debug_assert!(
                refreshed,
                "utterance {utt_idx} was in reusable set but refresh failed"
            );
        }
        utt_idx += 1;
    }
}

/// Enforce E362: strip timing from utterances whose start time is before
/// the previous utterance's start time (non-monotonic ordering).
///
/// Also truncates end-time overlaps: when utterance N's end exceeds
/// utterance N+1's start, N's end is clamped to N+1's start. Without
/// this, UTR's independent per-utterance bullet assignment produces
/// systematic ~1000ms overlaps where adjacent utterances claim
/// overlapping ASR token ranges from the global DP alignment.
///
/// Special case: if the clamped end would equal or precede the utterance's
/// own start (possible when two adjacent utterances share the same start_ms
/// from overlapping UTR token ranges), the bullet is stripped entirely rather
/// than left as a zero-duration `•T_T•` span, which would fail E362 validation.
pub fn enforce_monotonicity(
    chat_file: &mut ChatFile,
) -> Vec<talkbank_transform::decisions::DecisionRecord> {
    use talkbank_transform::decisions::DecisionRecord;

    let mut decisions = Vec::new();

    // Pass 1: strip utterances with non-monotonic start times.
    let mut last_start_ms: u64 = 0;
    for (line_idx, line) in chat_file.lines.iter_mut().enumerate() {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };
        match utt.main.content.bullet.as_ref().map(|b| b.timing.start_ms) {
            Some(s) if s < last_start_ms => {
                decisions.push(DecisionRecord::new_and_trace(
                    line_idx,
                    utt.main.speaker.as_str().to_string(),
                    talkbank_transform::decisions::DecisionStrategy::Monotonicity(
                        talkbank_transform::decisions::MonotonicityStrategy::TimingStripped,
                    ),
                    format!("non_monotonic start_ms={s} previous_start_ms={last_start_ms}"),
                    true,
                ));
                strip_utterance_timing(utt);
            }
            Some(s) => last_start_ms = s,
            None => {}
        }
    }

    // Pass 2: clamp end-time overlaps.
    let timed: Vec<(usize, u64)> = chat_file
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            if let Line::Utterance(u) = line {
                Some((i, u.main.content.bullet.as_ref()?.timing.start_ms))
            } else {
                None
            }
        })
        .collect();

    for pair in timed.windows(2) {
        let (prev_idx, _prev_start) = pair[0];
        let (_next_idx, next_start) = pair[1];

        if let Line::Utterance(prev_utt) = &mut chat_file.lines[prev_idx]
            && let Some(bullet) = prev_utt.main.content.bullet.as_ref()
            && bullet.timing.end_ms > next_start
        {
            let original_end = bullet.timing.end_ms;
            let overlap_ms = original_end - next_start;
            let start_ms = bullet.timing.start_ms;

            if next_start <= start_ms {
                // Clamping would produce a zero-or-negative-duration
                // bullet (next_start ≤ prev.start), which fails E362.
                // Strip the bullet entirely — untimed is safer than invalid.
                decisions.push(DecisionRecord::new_and_trace(
                    prev_idx,
                    prev_utt.main.speaker.as_str().to_string(),
                    talkbank_transform::decisions::DecisionStrategy::Monotonicity(
                        talkbank_transform::decisions::MonotonicityStrategy::TimingStripped,
                    ),
                    format!(
                        "zero_duration_clamp original_end={original_end} \
                                 next_start={next_start} start_ms={start_ms} \
                                 cause=utr_identical_start_times"
                    ),
                    true,
                ));
                strip_utterance_timing(prev_utt);
            } else {
                decisions.push(DecisionRecord::new_and_trace(
                    prev_idx,
                    prev_utt.main.speaker.as_str().to_string(),
                    talkbank_transform::decisions::DecisionStrategy::Monotonicity(
                        talkbank_transform::decisions::MonotonicityStrategy::EndClamped,
                    ),
                    format!(
                        "end_truncated_by={overlap_ms}ms original_end={original_end} \
                                 clamped_to={next_start} cause=utr_token_range_overlap"
                    ),
                    // `end_clamped` is routine housekeeping: a few-millisecond
                    // UTR overlap correction that prevents E362 validation
                    // errors.  It does NOT indicate an alignment defect and
                    // must not trigger %xrev (human review), which would
                    // mislead researchers into thinking correctly-aligned
                    // utterances need correction.  The %xalign audit record
                    // is still written; only the %xrev flag is suppressed.
                    // BA2 made these same corrections silently.
                    false,
                ));
                // Control-flow invariant: the enclosing
                // `if let Line::Utterance(prev_utt) ... && let Some(bullet)`
                // guard at line 261-263 already proved
                // `prev_utt.main.content.bullet` is `Some(...)`. The
                // intervening code never replaces or clears the bullet
                // before reaching this assignment.
                #[allow(clippy::unwrap_used)]
                {
                    prev_utt.main.content.bullet.as_mut().unwrap().timing.end_ms = next_start;
                }
            }
        }
    }

    decisions
}

/// Strip `%wor` tiers from utterances whose bullets were removed by
/// `enforce_monotonicity`.
///
/// ## Why this is necessary
///
/// `enforce_monotonicity` strips a main-tier bullet when it is non-monotonic
/// (backward relative to the previous utterance).  However, if the utterance
/// also carries a `%wor` tier with the same backward timestamps, the NEXT
/// re-run will enter the `has_reusable_wor_timing` fast path, call
/// `refresh_existing_alignment`, and reconstruct the backward bullet from
/// the stale `%wor` data — reintroducing the E362 violation.
///
/// This function removes the `%wor` tier from every utterance that
/// `enforce_monotonicity` stripped, breaking the re-run cycle:
///
/// ```text
/// bad %wor → fast path → backward bullet → enforce strips bullet
///          → strip_wor removes %wor → next run skips fast path
///          → full FA re-aligns → correct timing
/// ```
///
/// Call this immediately after `enforce_monotonicity` wherever the
/// `has_reusable_wor_timing` fast path is used:
/// - `run_fa_from_ast` fast-path return in `batchalign/src/fa/mod.rs`
///
/// `decisions` must be the slice returned by `enforce_monotonicity`.
/// Only decisions with `module == DecisionModule::Monotonicity` and
/// `strategy == "timing_stripped"` are processed; end-clamped utterances
/// keep their `%wor` because their timing is still monotonic.
pub fn strip_wor_from_monotonicity_stripped_utterances(
    chat_file: &mut ChatFile,
    decisions: &[talkbank_transform::decisions::DecisionRecord],
) {
    use talkbank_transform::decisions::{DecisionStrategy, MonotonicityStrategy};

    // Collect the line indices of utterances that had their bullets stripped
    // (as opposed to end-clamped, which still have valid timing).
    // Typically 0–2 utterances, so a small Vec is cheaper than a HashSet.
    let stripped: Vec<usize> = decisions
        .iter()
        .filter(|d| {
            matches!(
                d.strategy,
                DecisionStrategy::Monotonicity(MonotonicityStrategy::TimingStripped)
            )
        })
        .map(|d| d.line_idx)
        .collect();

    if stripped.is_empty() {
        return;
    }

    for (line_idx, line) in chat_file.lines.iter_mut().enumerate() {
        if !stripped.contains(&line_idx) {
            continue;
        }
        let Line::Utterance(utterance) = line else {
            continue;
        };
        // Remove the %wor tier.  The utterance will be re-aligned on the next
        // run through the full FA path rather than the fast path.
        super::remove_wor_tier(utterance);
    }
}

/// Enforce E704: strip timing from the EARLIER utterance when consecutive
/// same-speaker utterances overlap by more than 500ms tolerance.
pub fn strip_e704_same_speaker_overlaps(chat_file: &mut ChatFile) {
    const E704_TOLERANCE_MS: u64 = 500;

    let utt_info: Vec<(usize, String, u64, u64)> = chat_file
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            if let Line::Utterance(u) = line {
                let bullet = u.main.content.bullet.as_ref()?;
                let speaker = u.main.speaker.as_str().to_string();
                Some((i, speaker, bullet.timing.start_ms, bullet.timing.end_ms))
            } else {
                None
            }
        })
        .collect();

    let mut to_strip: Vec<usize> = Vec::new();
    let mut last_by_speaker: HashMap<String, (usize, u64)> = HashMap::new();

    for &(line_idx, ref speaker, start_ms, end_ms) in &utt_info {
        if let Some(&(prev_idx, prev_end)) = last_by_speaker.get(speaker.as_str())
            && prev_end > start_ms + E704_TOLERANCE_MS
        {
            to_strip.push(prev_idx);
        }
        last_by_speaker.insert(speaker.clone(), (line_idx, end_ms));
    }

    for idx in to_strip {
        if let Line::Utterance(utt) = &mut chat_file.lines[idx] {
            strip_utterance_timing(utt);
        }
    }
}

// ---------------------------------------------------------------------------
// Timing stripping helpers
// ---------------------------------------------------------------------------

/// Strip all timing information from utterance content items.
///
/// Removes `InternalBullet` items and clears `inline_bullet` from all words.
pub fn strip_timing_from_content(items: &mut Vec<UtteranceContent>) {
    items.retain(|item| !matches!(item, UtteranceContent::InternalBullet(_)));

    for item in items.iter_mut() {
        match item {
            UtteranceContent::Word(w) => {
                w.inline_bullet = None;
            }
            UtteranceContent::AnnotatedWord(aw) => {
                aw.inner.inline_bullet = None;
            }
            UtteranceContent::ReplacedWord(rw) => {
                rw.word.inline_bullet = None;
            }
            UtteranceContent::Group(g) => {
                strip_timing_from_bracketed(&mut g.content.content.0);
            }
            UtteranceContent::AnnotatedGroup(ag) => {
                strip_timing_from_bracketed(&mut ag.inner.content.content.0);
            }
            _ => {}
        }
    }
}

/// Remove parsed internal bullet tokens while preserving `Word.inline_bullet`.
///
/// This is used by the cheap rerun path after `%wor` timing is copied back to
/// main-tier words. Without this cleanup the serializer would emit both the
/// old parsed bullet tokens and the refreshed word-level bullets.
fn strip_internal_bullet_tokens(items: &mut Vec<UtteranceContent>) {
    items.retain(|item| !matches!(item, UtteranceContent::InternalBullet(_)));

    for item in items.iter_mut() {
        match item {
            UtteranceContent::Group(group) => {
                strip_internal_bullet_tokens_bracketed(&mut group.content.content.0);
            }
            UtteranceContent::AnnotatedGroup(group) => {
                strip_internal_bullet_tokens_bracketed(&mut group.inner.content.content.0);
            }
            _ => {}
        }
    }
}

fn strip_internal_bullet_tokens_bracketed(items: &mut Vec<BracketedItem>) {
    items.retain(|item| !matches!(item, BracketedItem::InternalBullet(_)));

    for item in items.iter_mut() {
        match item {
            BracketedItem::AnnotatedGroup(group) => {
                strip_internal_bullet_tokens_bracketed(&mut group.inner.content.content.0);
            }
            BracketedItem::PhoGroup(group) => {
                strip_internal_bullet_tokens_bracketed(&mut group.content.content.0);
            }
            BracketedItem::SinGroup(group) => {
                strip_internal_bullet_tokens_bracketed(&mut group.content.content.0);
            }
            BracketedItem::Quotation(group) => {
                strip_internal_bullet_tokens_bracketed(&mut group.content.content.0);
            }
            _ => {}
        }
    }
}

fn strip_timing_from_bracketed(items: &mut Vec<BracketedItem>) {
    items.retain(|item| !matches!(item, BracketedItem::InternalBullet(_)));

    for item in items.iter_mut() {
        match item {
            BracketedItem::Word(w) => {
                w.inline_bullet = None;
            }
            BracketedItem::AnnotatedWord(aw) => {
                aw.inner.inline_bullet = None;
            }
            BracketedItem::AnnotatedGroup(ag) => {
                strip_timing_from_bracketed(&mut ag.inner.content.content.0);
            }
            _ => {}
        }
    }
}

/// Collect a flat timing vector for main-tier Wor-alignable words by aligning
/// the existing `%wor` tier back onto the main tier.
fn collect_wor_backed_timings(utterance: &Utterance) -> Option<Vec<Option<WordTiming>>> {
    const MAX_REUSABLE_WOR_WORD_DURATION_PROPORTION: f64 = 0.4;
    const MIN_WORDS_FOR_DOMINANCE_CHECK: usize = 3;
    const MIN_REUSABLE_WOR_WORD_DURATION_MS: u64 = 40;

    let wor = utterance.wor_tier()?.clone();
    let sidecar = resolve_wor_timing_sidecar(&utterance.main, &wor);
    let count = sidecar.positional_count()?;
    if count != count_alignable_main_words(utterance) {
        return None;
    }

    let wor_words: Vec<_> = wor.words().collect();
    if wor_words.len() != count {
        return None;
    }
    let mut timings = Vec::with_capacity(count);

    for word in wor_words {
        let bullet = word.inline_bullet.as_ref()?;
        timings.push(Some(WordTiming::new(
            bullet.timing.start_ms,
            bullet.timing.end_ms,
        )));
    }

    if timings
        .iter()
        .flatten()
        .any(|span| span.duration_ms() < MIN_REUSABLE_WOR_WORD_DURATION_MS)
    {
        return None;
    }

    if timings.len() >= MIN_WORDS_FOR_DOMINANCE_CHECK {
        let mut first_start = None;
        let mut last_end = None;
        let mut max_duration_ms = 0u64;
        for span in timings.iter().flatten() {
            if first_start.is_none_or(|start| span.start_ms < start) {
                first_start = Some(span.start_ms);
            }
            if last_end.is_none_or(|end| span.end_ms > end) {
                last_end = Some(span.end_ms);
            }
            max_duration_ms = max_duration_ms.max(span.duration_ms());
        }
        let utterance_span_ms = last_end?.saturating_sub(first_start?);
        if utterance_span_ms > 0
            && (max_duration_ms as f64 / utterance_span_ms as f64)
                > MAX_REUSABLE_WOR_WORD_DURATION_PROPORTION
        {
            return None;
        }
    }

    Some(timings)
}

pub(super) fn collect_wor_backed_span(utterance: &Utterance) -> Option<WordTiming> {
    let timings = collect_wor_backed_timings(utterance)?;
    let mut first_start = None;
    let mut last_end = None;
    for span in timings.iter().flatten() {
        if first_start.is_none_or(|start| span.start_ms < start) {
            first_start = Some(span.start_ms);
        }
        if last_end.is_none_or(|end| span.end_ms > end) {
            last_end = Some(span.end_ms);
        }
    }
    Some(WordTiming::new(first_start?, last_end?))
}

/// Strip timing and %wor from a single utterance.
pub(super) fn strip_utterance_timing(utt: &mut Utterance) {
    utt.main.content.bullet = None;
    strip_timing_from_content(&mut utt.main.content.content.0);
    // Remove %wor tiers.
    utt.dependent_tiers
        .retain(|t| !matches!(t, DependentTier::Wor(_)));
}
