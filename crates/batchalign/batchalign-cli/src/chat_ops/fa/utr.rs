//! Utterance Timing Recovery (UTR): inject ASR-derived timing into untimed CHAT utterances.
//!
//! When a CHAT file has a mix of timed and untimed utterances, UTR uses ASR
//! output to recover utterance-level bullets for the untimed ones. This is a
//! pre-pass before forced alignment — FA then operates on a fully-timed file.
//!
//! Algorithm:
//! 1. Flatten ALL utterance words (timed + untimed) into one reference sequence,
//!    each tagged with its source utterance index.
//! 2. Try a cheap O(n+m) fast path: if the full transcript words are a
//!    *uniquely embedded* exact monotonic subsequence of the ASR token stream,
//!    use that directly.
//! 3. If the exact subsequence is missing or ambiguous, fall back to a single
//!    global Hirschberg DP alignment of all words against all ASR tokens
//!    (`dp_align::align`).
//! 4. Collect per-utterance min/max matched ASR token indices from the chosen
//!    alignment.
//! 5. Set `utterance.main.content.bullet` from matched tokens' time span
//!    (untimed only). Already-timed utterances are left unchanged.
//!
//! This global monotonic alignment is the current correctness boundary for UTR:
//! it fixes 407-style hand-edited transcript failures where earlier utterances
//! consumed tokens that later utterances needed. It does not make UTR
//! non-monotonic, so dense overlap / text-audio reordering cases can still
//! remain unmatched.

use talkbank_model::model::{Bullet, ChatFile, Line, Linker};

use talkbank_transform::dp_align::{self, MatchMode};

use super::extraction::collect_fa_words;

pub mod overlap_markers;
mod two_pass;

/// Synthetic drift-class regression scenarios. Public entry point is
/// [`inject_utr_timing`]; the scenarios generate CHAT + ASR in-memory and
/// assert monotonicity / non-silent-strip invariants on the output. The
/// four drift-inducing scenarios are `#[ignore]` because they are currently
/// RED on `GlobalUtr` (the default dispatch) — by design; they codify a
/// bug. Run explicitly with
/// `cargo test -p batchalign-chat-ops --lib drift_scenarios -- --ignored`.
#[cfg(test)]
mod drift_scenarios;

/// A single ASR token with timing, used as input for UTR.
///
/// This is intentionally a simple struct — it can be constructed from
/// any ASR response format (Python worker `AsrToken`, or any other source).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AsrTimingToken {
    /// Token text (single word).
    pub text: String,
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
}

/// Result summary from UTR injection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UtrResult {
    /// Utterances that received timing from ASR tokens.
    pub injected: usize,
    /// Already-timed utterances (left unchanged).
    pub skipped: usize,
    /// Untimed utterances that could not be matched to ASR tokens.
    pub unmatched: usize,
    /// Per-utterance decision records for unmatched utterances.
    /// Excluded from equality comparison and serialization — these are
    /// provenance metadata, not part of the UTR result semantics.
    #[serde(skip)]
    pub decisions: Vec<talkbank_transform::decisions::DecisionRecord>,
}

impl PartialEq for UtrResult {
    fn eq(&self, other: &Self) -> bool {
        self.injected == other.injected
            && self.skipped == other.skipped
            && self.unmatched == other.unmatched
    }
}

impl Eq for UtrResult {}

/// Strategy trait for UTR injection.
///
/// Implementations determine how ASR tokens are aligned with CHAT utterances
/// to recover utterance-level timing bullets.
pub trait UtrStrategy: Send + Sync {
    /// Inject utterance-level timing from ASR tokens into untimed CHAT utterances.
    fn inject(&self, chat_file: &mut ChatFile, asr_tokens: &[AsrTimingToken]) -> UtrResult;
}

/// Global single-pass UTR strategy.
///
/// Flattens all utterance words into one reference sequence and runs a single
/// alignment pass (exact-subsequence fast path or Hirschberg DP fallback).
/// This is the original UTR algorithm — monotonic, works well when transcript
/// order matches audio order, but cannot correctly place `+<` overlap
/// backchannels whose words appear at the wrong position in the global sequence.
pub struct GlobalUtr;

pub use two_pass::{
    CaMarkerPolicy, GroupingContext, TwoPassConfig, TwoPassOverlapUtr, UtrMatchMode,
};

/// Select the best UTR strategy for a given CHAT file.
///
/// Returns [`TwoPassOverlapUtr`] when any utterance has a `+<` lazy overlap
/// linker or CA overlap markers (⌊), [`GlobalUtr`] otherwise. When no overlap
/// utterances exist, pass 2 is a no-op, but we avoid the overhead entirely.
///
/// When `grouping_context` is provided, the two-pass strategy uses FA group
/// counts to detect and avoid the wider-window regression on non-English files.
pub fn select_strategy(
    chat_file: &ChatFile,
    grouping_context: Option<GroupingContext>,
) -> Box<dyn UtrStrategy> {
    let has_overlap = chat_file.lines.iter().any(|line| {
        if let Line::Utterance(utt) = line {
            // Check for +< linker (explicit overlap marker)
            if utt
                .main
                .content
                .linkers
                .0
                .contains(&Linker::LazyOverlapPrecedes)
            {
                return true;
            }
            // Check for ⌊ CA overlap markers (bottom overlap = overlapping speaker)
            let info = overlap_markers::extract_overlap_info(&utt.main.content.content.0);
            info.has_bottom_overlap()
        } else {
            false
        }
    });
    if has_overlap {
        Box::new(TwoPassOverlapUtr {
            grouping_context,
            config: two_pass::TwoPassConfig::default(),
        })
    } else {
        Box::new(GlobalUtr)
    }
}

/// Pre-extracted utterance metadata used while planning one UTR pass.
#[derive(Debug, Clone)]
pub(super) struct UtrUtteranceInfo {
    /// Alignable words from the utterance in transcript order.
    pub(super) words: Vec<String>,
    /// Whether the utterance already had a bullet before UTR.
    pub(super) has_bullet: bool,
    /// Whether the utterance has a `+<` lazy overlap linker.
    pub(super) has_lazy_overlap: bool,
    /// Whether this utterance contains ⌊ (bottom overlap) markers,
    /// indicating it overlaps with a preceding utterance's ⌈ markers.
    pub(super) has_ca_overlap: bool,
    /// For utterances with ⌈ (top overlap begin): the proportional position
    /// of the first ⌈ among the utterance's alignable words (0.0–1.0).
    /// Used by pass 2 to narrow the backchannel recovery window.
    pub(super) overlap_onset_fraction: Option<f64>,
    /// Speaker code for cross-utterance matching.
    pub(super) speaker: String,
    /// Indices of bottom overlap regions (for index-aware matching with
    /// predecessor tops). `None` = unindexed, `Some(n)` = indexed.
    pub(super) bottom_indices: Vec<Option<talkbank_model::model::OverlapIndex>>,
    /// Per-top-region onset fractions with their indices (for index-aware
    /// predecessor lookup).
    pub(super) top_onsets: Vec<(Option<talkbank_model::model::OverlapIndex>, f64)>,
}

/// Per-utterance matched ASR token range produced by one UTR alignment plan.
type UtrTokenRanges = Vec<Option<(usize, usize)>>;

/// Which alignment strategy produced the per-utterance token ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtrAlignmentStrategy {
    /// The entire transcript was a uniquely embedded exact monotonic
    /// subsequence of the ASR token stream, so DP was unnecessary.
    UniqueExactSubsequence,
    /// The fast path was impossible or ambiguous, so the full-file Hirschberg
    /// alignment remained necessary.
    GlobalDp,
}

/// Complete plan for one UTR alignment pass.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UtrAlignmentPlan {
    /// Strategy used to build the matched token ranges.
    strategy: UtrAlignmentStrategy,
    /// Per-utterance min/max matched token indices.
    utt_ranges: UtrTokenRanges,
}

/// Inject utterance-level timing from ASR tokens into untimed CHAT utterances.
///
/// Backward-compatible entry point that delegates to [`GlobalUtr`]. New callers
/// should prefer [`select_strategy`] to automatically choose the best strategy.
pub fn inject_utr_timing(chat_file: &mut ChatFile, asr_tokens: &[AsrTimingToken]) -> UtrResult {
    GlobalUtr.inject(chat_file, asr_tokens)
}

impl UtrStrategy for GlobalUtr {
    /// Global single-pass UTR: flatten all words, align once, assign bullets.
    ///
    /// Attempts a cheap exact-subsequence fast path first. Falls back to a
    /// single global Hirschberg DP alignment when the fast path is ambiguous.
    fn inject(&self, chat_file: &mut ChatFile, asr_tokens: &[AsrTimingToken]) -> UtrResult {
        run_global_utr(chat_file, asr_tokens, false, MatchMode::CaseInsensitive)
    }
}

/// Core global UTR implementation shared by [`GlobalUtr`] and the first pass
/// of [`TwoPassOverlapUtr`].
///
/// When `skip_lazy_overlap` is true, `+<` utterances are excluded from the
/// flattened word sequence (their words don't participate in the global DP)
/// but are still counted in the result as unmatched. Pass 2 of the two-pass
/// strategy handles them separately.
pub(super) fn run_global_utr(
    chat_file: &mut ChatFile,
    asr_tokens: &[AsrTimingToken],
    skip_lazy_overlap: bool,
    dp_match_mode: MatchMode,
) -> UtrResult {
    let mut result = UtrResult {
        injected: 0,
        skipped: 0,
        unmatched: 0,
        decisions: Vec::new(),
    };

    if asr_tokens.is_empty() {
        for line in &chat_file.lines {
            if let Line::Utterance(utt) = line {
                if utt.main.content.bullet.is_some() {
                    result.skipped += 1;
                } else {
                    result.unmatched += 1;
                }
            }
        }
        return result;
    }

    let utt_infos = collect_utr_utterance_info(chat_file);

    // Flatten utterance words into a single payload sequence, optionally
    // skipping overlap utterances (+< or ⌊-bearing) so the global alignment
    // sees only main-speaker words in their correct temporal order.
    let mut all_words: Vec<String> = Vec::new();
    let mut word_to_utt: Vec<usize> = Vec::new();
    for (utt_idx, info) in utt_infos.iter().enumerate() {
        if skip_lazy_overlap && (info.has_lazy_overlap || info.has_ca_overlap) {
            continue;
        }
        for word in &info.words {
            all_words.push(word.clone());
            word_to_utt.push(utt_idx);
        }
    }

    let asr_texts: Vec<String> = asr_tokens.iter().map(|t| t.text.clone()).collect();
    let plan = plan_utr_alignment(
        &all_words,
        &asr_texts,
        &word_to_utt,
        utt_infos.len(),
        dp_match_mode,
    );

    // Build utterance ordinal → line index mapping for decision records.
    let utt_line_indices: Vec<usize> = chat_file
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            if matches!(line, Line::Utterance(_)) {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    // Convert ranges to bullets for untimed utterances only.
    let mut bullets_to_set: Vec<Option<(u64, u64)>> = vec![None; utt_infos.len()];
    for (utt_idx, info) in utt_infos.iter().enumerate() {
        if info.has_bullet {
            result.skipped += 1;
            continue;
        }
        // +< utterances skipped in pass 1 get no range — count as unmatched
        // (the two-pass caller will handle them in pass 2).
        match plan.utt_ranges[utt_idx] {
            Some((min_asr, max_asr)) => {
                let start_ms = asr_tokens[min_asr].start_ms;
                let end_ms = asr_tokens[max_asr].end_ms;
                if start_ms < end_ms {
                    bullets_to_set[utt_idx] = Some((start_ms, end_ms));
                    result.injected += 1;
                } else {
                    // The matched ASR token range has start_ms >= end_ms — a
                    // zero-duration span produced by Whisper for very short words
                    // (single 20ms frame backchannels like "mhm", "yeah").
                    // Creating a •T_T• utterance bullet would be actively harmful:
                    // the FA postprocess bounds word timings to the utterance range,
                    // clamping every word timing to the empty [T,T] interval and
                    // dropping them, so the zero-duration bullet then perpetuates
                    // across every subsequent `align` re-run.
                    // Leave the utterance untimed; FA will assign a valid bullet
                    // from the word-level forced alignment instead.
                    result.unmatched += 1;
                    if let Some(&line_idx) = utt_line_indices.get(utt_idx)
                        && let Some(Line::Utterance(utt)) = chat_file.lines.get(line_idx)
                    {
                        result
                            .decisions
                            .push(talkbank_transform::decisions::DecisionRecord {
                                line_idx,
                                speaker: utt.main.speaker.as_str().to_string(),
                                strategy: talkbank_transform::decisions::DecisionStrategy::Utr(
                                    talkbank_transform::decisions::UtrStrategy::ZeroDurationSkipped,
                                ),
                                reason: format!(
                                    "words={} asr_range=[{min_asr},{max_asr}] \
                                     start_ms={start_ms} end_ms={end_ms} \
                                     reason=zero_or_negative_duration",
                                    info.words.len()
                                ),
                                needs_review: false,
                            });
                    }
                }
            }
            None => {
                result.unmatched += 1;
                // Record which utterance was unmatched.
                if let Some(&line_idx) = utt_line_indices.get(utt_idx)
                    && let Some(Line::Utterance(utt)) = chat_file.lines.get(line_idx)
                {
                    result
                        .decisions
                        .push(talkbank_transform::decisions::DecisionRecord {
                            line_idx,
                            speaker: utt.main.speaker.as_str().to_string(),
                            strategy: talkbank_transform::decisions::DecisionStrategy::Utr(
                                talkbank_transform::decisions::UtrStrategy::Unmatched,
                            ),
                            reason: format!("words={} no_asr_match", info.words.len()),
                            needs_review: true,
                        });
                }
            }
        }
    }

    // Post-pass: enforce strictly increasing start_ms for adjacent non-overlap
    // utterances among the bullets being set.
    //
    // The global DP can assign the same start_ms to two adjacent non-overlap
    // utterances when Whisper's 20ms DTW grid places consecutive short words
    // (e.g., "mhm" at 1000ms and "yeah" also at 1000ms) at the same token
    // boundary.  Without this fix, `enforce_monotonicity` pass 2 later clamps
    // prev.end → next.start = prev.start → zero-duration •T_T•, which fails
    // E362 validation and then perpetuates through every subsequent align re-run.
    //
    // Strategy: walk non-overlap utterances in document order, tracking
    // `floor_end_ms` — the end_ms of the last bullet we committed (either an
    // already-timed utterance's existing bullet, or a newly assigned one).  If a
    // newly assigned bullet starts before `floor_end_ms`, advance its start_ms
    // (and end_ms if necessary) so it begins strictly after the previous one ended.
    {
        // Pre-collect existing timing for already-timed utterances so we can
        // seed `floor_end_ms` when the first timed block precedes new ones.
        let existing_timing: Vec<Option<(u64, u64)>> = chat_file
            .lines
            .iter()
            .filter_map(|l| {
                if let Line::Utterance(u) = l {
                    Some(
                        u.main
                            .content
                            .bullet
                            .as_ref()
                            .map(|b| (b.timing.start_ms, b.timing.end_ms)),
                    )
                } else {
                    None
                }
            })
            .collect();

        let mut floor_end_ms: u64 = 0;
        for (utt_idx, info) in utt_infos.iter().enumerate() {
            // Overlap utterances legitimately share timing with the previous
            // utterance — do not advance their start or update the floor.
            if info.has_lazy_overlap || info.has_ca_overlap {
                continue;
            }
            if info.has_bullet {
                // Already-timed: update floor from existing bullet.
                if let Some(Some((_, end_ms))) = existing_timing.get(utt_idx) {
                    floor_end_ms = floor_end_ms.max(*end_ms);
                }
                continue;
            }
            if let Some((ref mut start_ms, ref mut end_ms)) = bullets_to_set[utt_idx] {
                if *start_ms < floor_end_ms {
                    // Advance start so this bullet begins after the previous one ended.
                    *start_ms = floor_end_ms;
                    if *end_ms <= *start_ms {
                        // Also extend end to preserve at least 1ms of duration.
                        *end_ms = *start_ms + 1;
                    }
                }
                floor_end_ms = *end_ms;
            }
        }
    }

    // Apply bullets to the actual ChatFile utterances
    let mut utt_idx = 0;
    for line in &mut chat_file.lines {
        if let Line::Utterance(utt) = line {
            if let Some((start_ms, end_ms)) = bullets_to_set[utt_idx] {
                // Mark as a provisional UTR hint so that update_utterance_bullet
                // (called after FA injection) overwrites this bullet with the
                // FA word span instead of union-expanding from it.
                utt.main.content.bullet = Some(Bullet::utr_hint(start_ms, end_ms));
            }
            utt_idx += 1;
        }
    }

    result
}

/// Extract alignable words, bullet presence, `+<` linker status, and CA
/// overlap marker info for every utterance in the order UTR sees them.
pub(super) fn collect_utr_utterance_info(chat_file: &ChatFile) -> Vec<UtrUtteranceInfo> {
    let mut utt_infos = Vec::new();
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            let mut words = Vec::new();
            collect_fa_words(&utt.main.content.content, &mut words);
            let has_lazy_overlap = utt
                .main
                .content
                .linkers
                .0
                .contains(&Linker::LazyOverlapPrecedes);
            let overlap_info = overlap_markers::extract_overlap_info(&utt.main.content.content.0);

            // Collect bottom region indices for index-aware matching.
            let bottom_indices: Vec<_> = overlap_info
                .regions
                .iter()
                .filter(|r| {
                    r.kind == talkbank_model::alignment::helpers::OverlapRegionKind::Bottom
                        && r.has_begin()
                })
                .map(|r| r.index)
                .collect();

            // Collect per-top-region onset fractions with their indices.
            let top_onsets: Vec<_> = overlap_info
                .regions
                .iter()
                .filter(|r| {
                    r.kind == talkbank_model::alignment::helpers::OverlapRegionKind::Top
                        && r.has_begin()
                })
                .filter_map(|r| {
                    let word_pos = r.begin_at_word?;
                    if overlap_info.total_words == 0 {
                        return None;
                    }
                    let fraction = word_pos as f64 / overlap_info.total_words as f64;
                    Some((r.index, fraction))
                })
                .collect();

            utt_infos.push(UtrUtteranceInfo {
                words,
                has_bullet: utt.main.content.bullet.is_some(),
                has_lazy_overlap,
                has_ca_overlap: overlap_info.has_bottom_overlap(),
                overlap_onset_fraction: overlap_info.top_onset_fraction(),
                speaker: utt.main.speaker.to_string(),
                bottom_indices,
                top_onsets,
            });
        }
    }
    utt_infos
}

/// Plan the per-utterance ASR token ranges for one UTR pass.
///
/// This first tries the cheap exact-subsequence fast path. If every transcript
/// word appears in ASR order *and* that embedding is unique, UTR can avoid DP.
/// Any missing word or repeated-token ambiguity falls back to the global
/// Hirschberg alignment.
fn plan_utr_alignment(
    all_words: &[String],
    asr_texts: &[String],
    word_to_utt: &[usize],
    utt_count: usize,
    dp_match_mode: MatchMode,
) -> UtrAlignmentPlan {
    // Exact-subsequence fast path only works with exact/case-insensitive matching.
    if matches!(dp_match_mode, MatchMode::Exact | MatchMode::CaseInsensitive)
        && let Some(utt_ranges) =
            try_unique_exact_subsequence_ranges(all_words, asr_texts, word_to_utt, utt_count)
    {
        return UtrAlignmentPlan {
            strategy: UtrAlignmentStrategy::UniqueExactSubsequence,
            utt_ranges,
        };
    }

    let alignment = dp_align::align(all_words, asr_texts, dp_match_mode);
    UtrAlignmentPlan {
        strategy: UtrAlignmentStrategy::GlobalDp,
        utt_ranges: collect_utt_ranges_from_alignment(&alignment, word_to_utt, utt_count),
    }
}

/// Attempt the exact-subsequence fast path for UTR.
///
/// The fast path is accepted only when the transcript words have exactly one
/// monotonic embedding into the ASR stream. Repeated-token ambiguity therefore
/// forces a DP fallback instead of silently accepting an arbitrary greedy path.
fn try_unique_exact_subsequence_ranges(
    all_words: &[String],
    asr_texts: &[String],
    word_to_utt: &[usize],
    utt_count: usize,
) -> Option<UtrTokenRanges> {
    let earliest = greedy_forward_match_indices(all_words, asr_texts)?;
    let latest = greedy_reverse_match_indices(all_words, asr_texts)?;
    if earliest != latest {
        return None;
    }

    Some(collect_utt_ranges_from_match_indices(
        &earliest,
        word_to_utt,
        utt_count,
    ))
}

/// Return the earliest monotonic exact-subsequence match indices for the
/// payload words.
fn greedy_forward_match_indices(payload: &[String], reference: &[String]) -> Option<Vec<usize>> {
    let mut reference_idx = 0;
    let mut matches = Vec::with_capacity(payload.len());

    for payload_word in payload {
        while reference_idx < reference.len()
            && !payload_word.eq_ignore_ascii_case(&reference[reference_idx])
        {
            reference_idx += 1;
        }
        if reference_idx == reference.len() {
            return None;
        }
        matches.push(reference_idx);
        reference_idx += 1;
    }

    Some(matches)
}

/// Return the latest monotonic exact-subsequence match indices for the payload
/// words.
fn greedy_reverse_match_indices(payload: &[String], reference: &[String]) -> Option<Vec<usize>> {
    let mut reference_idx = reference.len();
    let mut matches = vec![0; payload.len()];

    for (payload_idx, payload_word) in payload.iter().enumerate().rev() {
        let mut found = None;
        while reference_idx > 0 {
            reference_idx -= 1;
            if payload_word.eq_ignore_ascii_case(&reference[reference_idx]) {
                found = Some(reference_idx);
                break;
            }
        }
        matches[payload_idx] = found?;
    }

    Some(matches)
}

/// Convert one matched reference index per payload word into per-utterance
/// token ranges.
fn collect_utt_ranges_from_match_indices(
    matched_reference_indices: &[usize],
    word_to_utt: &[usize],
    utt_count: usize,
) -> UtrTokenRanges {
    let mut utt_ranges = vec![None; utt_count];
    for (payload_idx, reference_idx) in matched_reference_indices.iter().enumerate() {
        let utt_idx = word_to_utt[payload_idx];
        update_utt_range(&mut utt_ranges[utt_idx], *reference_idx);
    }
    utt_ranges
}

/// Convert Hirschberg alignment matches into per-utterance token ranges.
fn collect_utt_ranges_from_alignment(
    alignment: &[dp_align::AlignResult],
    word_to_utt: &[usize],
    utt_count: usize,
) -> UtrTokenRanges {
    let mut utt_ranges = vec![None; utt_count];
    for result_item in alignment {
        if let dp_align::AlignResult::Match {
            payload_idx,
            reference_idx,
            ..
        } = result_item
        {
            let utt_idx = word_to_utt[*payload_idx];
            update_utt_range(&mut utt_ranges[utt_idx], *reference_idx);
        }
    }
    utt_ranges
}

/// Extend one utterance token range to include an additional matched ASR index.
fn update_utt_range(utt_range: &mut Option<(usize, usize)>, reference_idx: usize) {
    match utt_range {
        Some((min_idx, max_idx)) => {
            if reference_idx < *min_idx {
                *min_idx = reference_idx;
            }
            if reference_idx > *max_idx {
                *max_idx = reference_idx;
            }
        }
        None => {
            *utt_range = Some((reference_idx, reference_idx));
        }
    }
}

/// Cache key for a full-file UTR ASR result.
///
/// Key = BLAKE3("utr_asr|{audio_identity}|{lang}").
pub fn utr_asr_cache_key(
    audio_identity: &super::AudioIdentity,
    lang: &str,
) -> crate::chat_ops::CacheKey {
    let input = format!("utr_asr|{}|{lang}", audio_identity.as_str());
    crate::chat_ops::CacheKey::from_content(&input)
}

/// Cache key for a segment-level UTR ASR result (partial-window mode).
///
/// Key = BLAKE3("utr_asr_segment|{audio_identity}|{start_ms}|{end_ms}|{lang}").
pub fn utr_asr_segment_cache_key(
    audio_identity: &super::AudioIdentity,
    start_ms: u64,
    end_ms: u64,
    lang: &str,
) -> crate::chat_ops::CacheKey {
    let input = format!(
        "utr_asr_segment|{}|{start_ms}|{end_ms}|{lang}",
        audio_identity.as_str()
    );
    crate::chat_ops::CacheKey::from_content(&input)
}

/// Identify audio windows covering untimed utterances.
///
/// Each window spans from the preceding timed utterance's end to the
/// following timed utterance's start (with `padding_ms` on each side).
/// Adjacent untimed utterances are merged into a single window.
pub fn find_untimed_windows(
    chat_file: &ChatFile,
    total_audio_ms: u64,
    padding_ms: u64,
) -> Vec<(u64, u64)> {
    // Collect bullet info for each utterance in order
    let mut utt_bullets: Vec<Option<(u64, u64)>> = Vec::new();
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            utt_bullets.push(
                utt.main
                    .content
                    .bullet
                    .as_ref()
                    .map(|b| (b.timing.start_ms, b.timing.end_ms)),
            );
        }
    }

    if utt_bullets.is_empty() {
        return Vec::new();
    }

    // Find contiguous runs of untimed utterances and compute their windows
    let mut raw_windows: Vec<(u64, u64)> = Vec::new();
    let mut i = 0;
    while i < utt_bullets.len() {
        if utt_bullets[i].is_some() {
            i += 1;
            continue;
        }

        // Start of an untimed run
        let run_start = i;
        while i < utt_bullets.len() && utt_bullets[i].is_none() {
            i += 1;
        }
        // run_start..i is the untimed run

        // Window start: end of preceding timed utterance, or 0
        let window_start = if run_start > 0 {
            // Search backward for the nearest timed utterance
            (0..run_start)
                .rev()
                .find_map(|j| utt_bullets[j].map(|(_, end)| end))
                .unwrap_or(0)
        } else {
            0
        };

        // Window end: start of following timed utterance, or total_audio_ms
        let window_end = if i < utt_bullets.len() {
            // Search forward for the nearest timed utterance
            (i..utt_bullets.len())
                .find_map(|j| utt_bullets[j].map(|(start, _)| start))
                .unwrap_or(total_audio_ms)
        } else {
            total_audio_ms
        };

        // Apply padding and clamp
        let padded_start = window_start.saturating_sub(padding_ms);
        let padded_end = (window_end + padding_ms).min(total_audio_ms);

        raw_windows.push((padded_start, padded_end));
    }

    // Merge overlapping windows
    if raw_windows.is_empty() {
        return Vec::new();
    }
    raw_windows.sort_by_key(|&(start, _)| start);
    let mut merged: Vec<(u64, u64)> = vec![raw_windows[0]];
    for &(start, end) in &raw_windows[1..] {
        // SAFETY: `merged` is initialized with `vec![raw_windows[0]]`, so it is
        // always non-empty at this point.
        #[allow(clippy::unwrap_used)]
        let last = merged.last_mut().unwrap();
        if start <= last.1 {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_parser::TreeSitterParser;

    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().unwrap();
        parser.parse_chat_file(text).unwrap()
    }

    fn make_asr_tokens(words_with_times: &[(&str, u64, u64)]) -> Vec<AsrTimingToken> {
        words_with_times
            .iter()
            .map(|(text, start, end)| AsrTimingToken {
                text: text.to_string(),
                start_ms: *start,
                end_ms: *end,
            })
            .collect()
    }

    /// Captured old-batchalign output for the trimmed 407 regression fixture.
    ///
    /// The fixture stores one row per utterance from the legacy reference
    /// output so the regression test can verify both coverage and timing
    /// neighborhood, not only the presence of a bullet.
    #[derive(Debug, serde::Deserialize)]
    struct ExpectedUtrFixture {
        /// Speaker code from the captured reference output.
        speaker: String,
        /// Expected utterance start time in milliseconds, when one existed.
        start_ms: Option<u64>,
        /// Expected utterance end time in milliseconds, when one existed.
        end_ms: Option<u64>,
        /// Main-tier text from the captured reference output.
        text: String,
    }

    /// Return `true` when two utterance spans land in the same timing
    /// neighborhood.
    ///
    /// Exact equality is too brittle for ASR-derived regression fixtures, but
    /// a restored global-DP UTR pass should still land on substantially the same
    /// interval. The spans must therefore either overlap or both endpoints must
    /// be within a small tolerance.
    fn spans_roughly_agree(actual: (u64, u64), expected: (u64, u64)) -> bool {
        const ENDPOINT_TOLERANCE_MS: u64 = 1_500;

        let overlaps = actual.0 <= expected.1 && expected.0 <= actual.1;
        let start_close = actual.0.abs_diff(expected.0) <= ENDPOINT_TOLERANCE_MS;
        let end_close = actual.1.abs_diff(expected.1) <= ENDPOINT_TOLERANCE_MS;

        overlaps || (start_close && end_close)
    }

    #[test]
    fn test_inject_utr_all_timed_is_noop() {
        let input = include_str!("../../../../../../resources/fixtures/fa_two_timed_utterances.cha");
        let mut chat = parse_chat(input);
        let tokens = make_asr_tokens(&[("hello", 0, 500), ("world", 600, 1000)]);
        let result = inject_utr_timing(&mut chat, &tokens);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.injected, 0);
        assert_eq!(result.unmatched, 0);
    }

    #[test]
    fn test_inject_utr_empty_tokens() {
        let input = include_str!("../../../../../../resources/fixtures/fa_two_untimed_with_media.cha");
        let mut chat = parse_chat(input);
        let result = inject_utr_timing(&mut chat, &[]);
        assert_eq!(result.unmatched, 2);
        assert_eq!(result.injected, 0);
    }

    #[test]
    fn test_inject_utr_untimed_gets_timing() {
        // Use a file with mixed timed/untimed utterances
        let input =
            include_str!("../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");
        let mut chat = parse_chat(input);

        // Count before
        let (timed_before, untimed_before) = super::super::grouping::count_utterance_timing(&chat);
        assert!(untimed_before > 0, "test fixture should have untimed utts");

        // Build ASR tokens matching the fixture's actual words:
        // utt 0 (timed): "the cat is here"
        // utt 1 (untimed): "she is looking outside"
        // utt 2 (timed): "there is a path"
        // utt 3 (untimed): "I do not know"
        // utt 4 (untimed): "but there is a building"
        // utt 5 (timed): "okay so now"
        let tokens = make_asr_tokens(&[
            // utt 0 (timed): cursor advance
            ("the", 10000, 10500),
            ("cat", 10600, 11000),
            ("is", 11200, 11500),
            ("here", 12000, 13000),
            // utt 1 (untimed): "she is looking outside"
            ("she", 15500, 16000),
            ("is", 16200, 16500),
            ("looking", 16800, 17500),
            ("outside", 17800, 18500),
            // utt 2 (timed): cursor advance
            ("there", 20500, 21000),
            ("is", 21200, 21500),
            ("a", 21800, 22000),
            ("path", 22200, 23000),
            // utt 3 (untimed): "I do not know"
            ("I", 26000, 26500),
            ("do", 26800, 27000),
            ("not", 27200, 27500),
            ("know", 27800, 28500),
            // utt 4 (untimed): "but there is a building"
            ("but", 30000, 30500),
            ("there", 30800, 31200),
            ("is", 31500, 31800),
            ("a", 32000, 32200),
            ("building", 32500, 33500),
            // utt 5 (timed): cursor advance
            ("okay", 40500, 41000),
            ("so", 41200, 41500),
            ("now", 41800, 42500),
        ]);

        let result = inject_utr_timing(&mut chat, &tokens);
        assert_eq!(result.skipped, 3, "3 already-timed utterances");
        assert_eq!(result.injected, 3, "3 untimed utterances should get timing");
        assert_eq!(result.unmatched, 0);

        // Verify all utterances now have bullets
        let (timed_after, untimed_after) = super::super::grouping::count_utterance_timing(&chat);
        assert_eq!(untimed_after, 0, "all utterances should now be timed");
        assert_eq!(timed_after, timed_before + untimed_before);
    }

    #[test]
    fn test_plan_utr_alignment_uses_unique_exact_subsequence_fast_path() {
        let all_words = vec![
            "the".to_string(),
            "cat".to_string(),
            "sat".to_string(),
            "down".to_string(),
        ];
        let asr_texts = vec![
            "noise".to_string(),
            "the".to_string(),
            "cat".to_string(),
            "sat".to_string(),
            "down".to_string(),
            "tail".to_string(),
        ];
        let word_to_utt = vec![0, 0, 1, 1];

        let plan = plan_utr_alignment(
            &all_words,
            &asr_texts,
            &word_to_utt,
            2,
            MatchMode::CaseInsensitive,
        );

        assert_eq!(plan.strategy, UtrAlignmentStrategy::UniqueExactSubsequence);
        assert_eq!(plan.utt_ranges, vec![Some((1, 2)), Some((3, 4))]);
    }

    #[test]
    fn test_plan_utr_alignment_falls_back_to_dp_when_exact_match_is_ambiguous() {
        let all_words = vec!["hello".to_string(), "world".to_string()];
        let asr_texts = vec![
            "hello".to_string(),
            "noise".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ];
        let word_to_utt = vec![0, 0];

        let plan = plan_utr_alignment(
            &all_words,
            &asr_texts,
            &word_to_utt,
            1,
            MatchMode::CaseInsensitive,
        );

        assert_eq!(plan.strategy, UtrAlignmentStrategy::GlobalDp);
        let range = plan.utt_ranges[0].expect("DP fallback should still time the utterance");
        assert_eq!(range.1, 3, "Range should reach the aligned final token");
    }

    #[test]
    fn test_count_utterance_timing() {
        let input =
            include_str!("../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");
        let chat = parse_chat(input);
        let (timed, untimed) = super::super::grouping::count_utterance_timing(&chat);
        assert_eq!(timed, 3);
        assert_eq!(untimed, 3);
    }

    #[test]
    fn test_count_utterance_timing_all_timed() {
        let input = include_str!("../../../../../../resources/fixtures/fa_two_timed_utterances.cha");
        let chat = parse_chat(input);
        let (timed, untimed) = super::super::grouping::count_utterance_timing(&chat);
        assert_eq!(timed, 2);
        assert_eq!(untimed, 0);
    }

    /// Regression test: a real-world trimmed UTR fixture from a hand-edited
    /// transcript failure report.
    ///
    /// This is the real token-starvation case: earlier utterances used
    /// to consume ASR tokens that later utterances needed. The regression must
    /// therefore prove two things:
    ///
    /// 1. coverage matches the known-good legacy output, and
    /// 2. recovered bullets still land in the same timing neighborhood.
    #[test]
    fn test_utr_real_world_trimmed_regression() {
        let chat_input =
            include_str!("../../../../../../resources/fixtures/utr_real_world_regression_input.cha");
        let tokens_json =
            include_str!("../../../../../../resources/fixtures/utr_real_world_regression_tokens.json");
        let expected_json =
            include_str!("../../../../../../resources/fixtures/utr_real_world_regression_expected.json");

        let mut chat = parse_chat(chat_input);
        let tokens: Vec<AsrTimingToken> = serde_json::from_str(tokens_json).unwrap();

        let expected: Vec<ExpectedUtrFixture> = serde_json::from_str(expected_json).unwrap();

        let result = inject_utr_timing(&mut chat, &tokens);

        // Collect actual timing
        let mut actual_timing: Vec<Option<(u64, u64)>> = Vec::new();
        for line in &chat.lines {
            if let Line::Utterance(utt) = line {
                actual_timing.push(
                    utt.main
                        .content
                        .bullet
                        .as_ref()
                        .map(|b| (b.timing.start_ms, b.timing.end_ms)),
                );
            }
        }

        assert_eq!(
            actual_timing.len(),
            expected.len(),
            "utterance count should match"
        );

        // Check each utterance
        let mut timed_count = 0;
        let mut coverage_regressions = Vec::new();
        let mut timing_regressions = Vec::new();
        for (i, (actual, exp)) in actual_timing.iter().zip(expected.iter()).enumerate() {
            let old_had_timing = exp.start_ms.is_some();
            let new_has_timing = actual.is_some();

            if new_has_timing {
                timed_count += 1;
            }

            if old_had_timing && !new_has_timing {
                coverage_regressions.push(format!(
                    "  U{} {}: expected {}-{}, ba3 has NONE: {}",
                    i + 1,
                    exp.speaker,
                    exp.start_ms.unwrap(),
                    exp.end_ms.unwrap(),
                    &exp.text[..exp.text.len().min(60)]
                ));
                continue;
            }

            if let (Some(actual), Some(start_ms), Some(end_ms)) = (actual, exp.start_ms, exp.end_ms)
                && !spans_roughly_agree(*actual, (start_ms, end_ms))
            {
                timing_regressions.push(format!(
                    "  U{} {}: expected {}-{}, ba3 got {}-{}: {}",
                    i + 1,
                    exp.speaker,
                    start_ms,
                    end_ms,
                    actual.0,
                    actual.1,
                    &exp.text[..exp.text.len().min(60)]
                ));
            }
        }

        let old_timed = expected.iter().filter(|e| e.start_ms.is_some()).count();

        // The goal: match or exceed old batchalign's coverage.
        // Old batchalign: 53/54 timed. We allow at most 1 regression.
        assert!(
            timed_count >= old_timed,
            "UTR regression: old batchalign timed {old_timed}/{total} utterances, \
             ba3 only timed {timed_count}/{total}.\n\
             Regressions ({n_reg}):\n{details}",
            total = expected.len(),
            n_reg = coverage_regressions.len(),
            details = coverage_regressions.join("\n")
        );

        assert!(
            timing_regressions.is_empty(),
            "UTR timing regression on 407 trimmed fixture.\n\
             Expected the restored global-DP path to stay in the same timing \
             neighborhood as the captured reference output.\n\
             Regressions ({n_reg}):\n{details}",
            n_reg = timing_regressions.len(),
            details = timing_regressions.join("\n")
        );

        // Also verify the overall result counters are consistent
        assert_eq!(
            result.injected + result.skipped + result.unmatched,
            expected.len(),
            "result counters should sum to total utterances"
        );
    }

    #[test]
    fn test_utr_asr_cache_key_deterministic() {
        use super::super::AudioIdentity;
        let identity = AudioIdentity::from_metadata("/tmp/audio.wav", 1234, 5678);
        let a = super::utr_asr_cache_key(&identity, "eng");
        let b = super::utr_asr_cache_key(&identity, "eng");
        assert_eq!(a, b);
    }

    #[test]
    fn test_utr_asr_cache_key_differs_for_different_inputs() {
        use super::super::AudioIdentity;
        let id1 = AudioIdentity::from_metadata("/tmp/a.wav", 1234, 5678);
        let id2 = AudioIdentity::from_metadata("/tmp/b.wav", 1234, 5678);
        let key1 = super::utr_asr_cache_key(&id1, "eng");
        let key2 = super::utr_asr_cache_key(&id2, "eng");
        assert_ne!(key1, key2, "different audio should produce different keys");

        let key3 = super::utr_asr_cache_key(&id1, "spa");
        assert_ne!(key1, key3, "different lang should produce different keys");
    }

    #[test]
    fn test_utr_asr_segment_cache_key_differs_for_windows() {
        use super::super::AudioIdentity;
        let identity = AudioIdentity::from_metadata("/tmp/audio.wav", 1234, 5678);
        let a = super::utr_asr_segment_cache_key(&identity, 0, 5000, "eng");
        let b = super::utr_asr_segment_cache_key(&identity, 5000, 10000, "eng");
        assert_ne!(a, b, "different windows should produce different keys");
    }

    #[test]
    fn test_find_untimed_windows_all_timed() {
        let input = include_str!("../../../../../../resources/fixtures/fa_two_timed_utterances.cha");
        let chat = parse_chat(input);
        let windows = super::find_untimed_windows(&chat, 60000, 500);
        assert!(windows.is_empty(), "all timed → no windows");
    }

    #[test]
    fn test_find_untimed_windows_mixed() {
        let input =
            include_str!("../../../../../../resources/fixtures/fa_mixed_timed_untimed_interleaved.cha");
        let chat = parse_chat(input);
        // This fixture has 3 timed and 3 untimed utterances interleaved
        let windows = super::find_untimed_windows(&chat, 60000, 500);
        assert!(!windows.is_empty(), "should find untimed windows");
        // Windows should be non-overlapping and ordered
        for w in windows.windows(2) {
            assert!(w[0].1 <= w[1].0, "windows should be non-overlapping");
        }
    }

    #[test]
    fn test_find_untimed_windows_all_untimed() {
        let input = include_str!("../../../../../../resources/fixtures/fa_two_untimed_with_media.cha");
        let chat = parse_chat(input);
        let windows = super::find_untimed_windows(&chat, 30000, 500);
        assert_eq!(windows.len(), 1, "all untimed → one merged window");
        assert_eq!(windows[0].0, 0, "starts at 0");
        assert_eq!(windows[0].1, 30000, "ends at total_audio_ms");
    }
}
