//! Forced alignment orchestration for CHAT files.
//!
//! Extracts pure-Rust FA logic from the PyO3 bridge (`batchalign-core`) so that
//! both the PyO3 layer and the root Rust workspace can share it.
//!
//! Pipeline: parse -> group utterances -> dispatch inference -> parse responses
//! -> inject timings -> postprocess -> generate %wor -> enforce monotonicity/E704.

pub mod alignment;
mod expand_for_fillers;
mod extraction;
mod grouping;
mod injection;
mod orchestrate;
pub mod outcome;
mod postprocess;
pub mod repair;
mod rescue_narrow_bullets;
pub mod review_tiers;
pub mod utr;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use talkbank_model::alignment::helpers::{TierDomain, WordItem, counts_for_tier, walk_words};
use talkbank_model::model::{
    Bullet, ChatFile, DependentTier, Line, Utterance, UtteranceContent, Word, WordCategory,
};
use talkbank_model::{UtteranceIdx, WordIdx};

// Re-export public API so that `crate::chat_ops::fa::Foo` paths remain unchanged.
pub use self::alignment::parse_fa_response;
pub use self::expand_for_fillers::expand_bullets_for_edge_fillers;
pub use self::extraction::collect_fa_words;
pub use self::grouping::{
    WHISPER_FA_MAX_LABEL_TOKENS, count_utterance_timing, estimate_untimed_boundaries,
    group_utterances,
};
pub use self::injection::inject_timings_for_utterance;
pub use self::orchestrate::{
    apply_fa_results, enforce_monotonicity, has_reusable_wor_timing_for_utterance,
    refresh_existing_alignment, refresh_existing_alignment_for_utterance,
    refresh_reusable_utterances, strip_e704_same_speaker_overlaps, strip_timing_from_content,
    strip_wor_from_monotonicity_stripped_utterances,
};
pub use self::postprocess::postprocess_utterance_timings;
pub use self::repair::{RepairDecision, RepairResult, RepairStats, repair_bullets};
pub use self::rescue_narrow_bullets::rescue_narrow_bullets;
pub use self::review_tiers::{ReviewLevel, inject_review_tiers};
pub use self::utr::{
    CaMarkerPolicy, GlobalUtr, GroupingContext, TwoPassConfig, TwoPassOverlapUtr, UtrMatchMode,
    UtrStrategy, find_untimed_windows, select_strategy, utr_asr_cache_key,
    utr_asr_segment_cache_key,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A time interval in milliseconds, guaranteed start <= end at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSpan {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
}

impl TimeSpan {
    /// Create a new time span. Caller is responsible for ensuring start <= end.
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        Self { start_ms, end_ms }
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// A timing result for a single word (alias for [`TimeSpan`]).
pub type WordTiming = TimeSpan;

/// Split a compound filler's cleaned text at underscores, or return the
/// text as a single element. Only applies to `WordCategory::Filler` words;
/// regular compounds are unchanged.
///
/// Both extraction and injection must agree on the split count — extraction
/// sends N parts to FA, injection consumes N timings from the cursor. This
/// shared helper is the single source of truth for the splitting rule.
pub fn split_compound_filler(word: &talkbank_model::model::Word) -> Vec<String> {
    use talkbank_model::model::WordCategory;
    let text = word.cleaned_text();
    if word.category == Some(WordCategory::Filler) && text.contains('_') {
        text.split('_')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![text.to_string()]
    }
}

/// A word extracted for forced alignment, with its position in the AST.
#[derive(Debug, Clone)]
pub struct FaWord {
    /// Index of the utterance in the file (among utterances only).
    pub utterance_index: UtteranceIdx,
    /// Index among alignable words within the utterance.
    pub utterance_word_index: WordIdx,
    /// Cleaned text for the FA model.
    pub text: String,
}

impl FaWord {
    /// Stable word identifier for callback protocols.
    pub fn stable_id(&self) -> String {
        format!("u{}:w{}", self.utterance_index, self.utterance_word_index)
    }
}

/// A group of utterances clustered for a single FA call.
#[derive(Debug)]
pub struct FaGroup {
    /// Audio window for this group.
    #[allow(dead_code)]
    pub audio_span: TimeSpan,
    /// Words in this group with positional indices.
    pub words: Vec<FaWord>,
    /// Utterance indices included in this group.
    pub utterance_indices: Vec<UtteranceIdx>,
}

impl FaGroup {
    /// Start of the audio window (ms).
    pub fn audio_start_ms(&self) -> u64 {
        self.audio_span.start_ms
    }

    /// End of the audio window (ms).
    pub fn audio_end_ms(&self) -> u64 {
        self.audio_span.end_ms
    }
}

/// Controls how word end times are set during FA post-processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaTimingMode {
    /// End of each word = start of next word (no silence between words).
    /// Used when the FA engine returns onset-only times (Wave2Vec).
    Continuous,
    /// End of each word = engine-provided end time (preserves pauses).
    /// Used when the FA engine returns word-level start+end (Whisper).
    WithPauses,
}

/// Wire type for the FA infer protocol -- one group sent to a Python worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaInferItem {
    /// Words to align (cleaned text).
    pub words: Vec<String>,
    /// Stable word IDs aligned 1:1 with `words`.
    pub word_ids: Vec<String>,
    /// Utterance indices aligned 1:1 with `words`.
    pub word_utterance_indices: Vec<usize>,
    /// Word indices inside each utterance aligned 1:1 with `words`.
    pub word_utterance_word_indices: Vec<usize>,
    /// Path to the audio file.
    pub audio_path: String,
    /// Start of the audio window (ms).
    pub audio_start_ms: u64,
    /// End of the audio window (ms).
    pub audio_end_ms: u64,
    /// How to handle word end times during post-processing.
    pub timing_mode: FaTimingMode,
}

impl FaInferItem {
    /// Audio window as a [`TimeSpan`].
    pub fn audio_span(&self) -> TimeSpan {
        TimeSpan::new(self.audio_start_ms, self.audio_end_ms)
    }
}

/// The forced alignment engine that produced word timings.
///
/// Determines how FA responses are interpreted:
/// - WhisperFa returns token-level onset times → requires DP alignment
/// - Wave2Vec returns word-level start+end pairs → index-aligned
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaEngineType {
    /// Whisper token-level FA. Onset times only; Hirschberg DP alignment
    /// maps tokens to words.
    WhisperFa,
    /// Wav2Vec word-level FA. Start+end times per word, 1:1 index-aligned
    /// with input words.
    Wave2Vec,
}

impl FaEngineType {
    /// Wire-format string for cache keys and serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WhisperFa => "whisper_fa",
            Self::Wave2Vec => "wave2vec",
        }
    }

    /// Parse from a wire-format string.
    ///
    /// Matches both `"wav2vec"` and `"wave2vec"` spellings, since the CLI
    /// generates `"wav2vec_fa"` while some older code used `"wave2vec"`.
    pub fn from_str_lossy(s: &str) -> Self {
        if s.contains("wav2vec") || s.contains("wave2vec") {
            Self::Wave2Vec
        } else {
            Self::WhisperFa
        }
    }
}

// ---------------------------------------------------------------------------
// %wor tier management
// ---------------------------------------------------------------------------

/// Remove existing %wor tier from an utterance (if any).
pub fn remove_wor_tier(utterance: &mut Utterance) {
    utterance
        .dependent_tiers
        .retain(|t| !matches!(t, DependentTier::Wor(_)));
}

/// Add a `%wor` tier generated from the inline bullets on words.
///
/// If the utterance already has a `%wor` tier, it is replaced **in place**,
/// preserving its position among other dependent tiers. If it has none,
/// the new tier is appended. Previously this called
/// [`remove_wor_tier`] followed by `push`, which destroyed the original
/// position and produced noisy git diffs on every file whose `%wor`
/// originally sat before `%mor` / `%gra` or in some other non-default
/// slot. The position-preserving replace fixes that entire class of
/// diff noise.
pub fn add_wor_tier(utterance: &mut Utterance) {
    let wor_tier = utterance.main.generate_wor_tier();
    talkbank_transform::inject::replace_or_add_tier(
        &mut utterance.dependent_tiers,
        DependentTier::Wor(wor_tier),
    );
}

/// Return `true` when every alignable FA word in the file already has reusable
/// `%wor` timing.
///
/// This intentionally does **not** look at `main` tier `inline_bullet` alone.
/// After a parse roundtrip, main-tier word timing may be represented as
/// `InternalBullet` tokens, while `%wor` carries the durable first-class timing
/// bullets. For the cheap rerun path we therefore verify that `%wor` fully and
/// cleanly aligns back to the main tier.
pub fn has_reusable_wor_timing(chat_file: &ChatFile) -> bool {
    let mut saw_alignable_word = false;
    let reusable = find_reusable_utterance_indices(chat_file);
    let mut expected_reusable = 0usize;
    let mut utt_idx = 0usize;

    for line in &chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };

        let main_word_count = count_alignable_main_words(utterance);
        if main_word_count == 0 {
            utt_idx += 1;
            continue;
        }
        saw_alignable_word = true;
        expected_reusable += 1;
        if !reusable.contains(&utt_idx) {
            return false;
        }
        utt_idx += 1;
    }

    saw_alignable_word && reusable.len() == expected_reusable
}

/// Find utterance indices that have reusable `%wor` timing.
///
/// Returns a set of utterance ordinal indices where
/// [`has_reusable_wor_timing_for_utterance()`] succeeds. Used by the plain
/// rerun path to selectively skip FA for utterances whose `%wor` is still
/// clean after manual edits to other utterances.
pub fn find_reusable_utterance_indices(chat_file: &ChatFile) -> std::collections::HashSet<usize> {
    struct ReuseCandidate {
        utt_idx: usize,
        has_alignable_words: bool,
        wor_span: Option<TimeSpan>,
        main_start_ms: Option<u64>,
    }

    let mut candidates = Vec::new();
    let mut reusable = std::collections::HashSet::new();
    let mut utt_idx = 0usize;
    for line in &chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        let has_alignable_words = count_alignable_main_words(utterance) > 0;
        candidates.push(ReuseCandidate {
            utt_idx,
            has_alignable_words,
            wor_span: if has_alignable_words {
                self::orchestrate::collect_wor_backed_span(utterance)
            } else {
                None
            },
            main_start_ms: utterance
                .main
                .content
                .bullet
                .as_ref()
                .map(|bullet| bullet.timing.start_ms),
        });
        utt_idx += 1;
    }

    let mut next_timed_start = None;
    let mut next_timed_start_after = vec![None; candidates.len()];
    for i in (0..candidates.len()).rev() {
        next_timed_start_after[i] = next_timed_start;
        if let Some(start_ms) = candidates[i].main_start_ms {
            next_timed_start = Some(start_ms);
        }
    }

    for (i, candidate) in candidates.iter().enumerate() {
        if !candidate.has_alignable_words {
            continue;
        }
        let Some(span) = candidate.wor_span else {
            continue;
        };
        if let Some(next_start_ms) = next_timed_start_after[i]
            && span.end_ms > next_start_ms
        {
            continue;
        }
        reusable.insert(candidate.utt_idx);
    }

    reusable
}

/// Count Wor-alignable words in the main tier.
pub(crate) fn count_alignable_main_words(utterance: &Utterance) -> usize {
    let mut count = 0usize;
    walk_words(
        &utterance.main.content.content,
        None,
        &mut |leaf| match leaf {
            WordItem::Word(word) => {
                if counts_for_tier(word, TierDomain::Wor) {
                    count += 1;
                }
            }
            WordItem::ReplacedWord(replaced) => {
                // Must mirror extraction policy: the original word is the FA
                // unit, not the replacement words.  Counting replacement words
                // here would overcount vs. extraction and desync the cursor.
                if counts_for_tier(&replaced.word, TierDomain::Wor) {
                    count += 1;
                }
            }
            WordItem::Separator(_) => {}
        },
    );
    count
}

/// Update the utterance-level bullet from word timings.
///
/// The behavior depends on the bullet's provenance ([`BulletSource`]):
///
/// - **No pre-existing bullet** — sets bullet directly from the FA word span.
///
/// - **`BulletSource::Utr`** — overwrites with the FA word span. The UTR
///   bullet was a provisional grouping hint; once FA has produced word
///   timings the hint is discarded and the FA span is authoritative.
///
/// - **`BulletSource::Authoritative`** — usually unions the existing bullet
///   with the FA word span when that preserves plausible leading/trailing
///   coverage. On reruns with an existing `%wor` tier, a large lead before the
///   first aligned word is only preserved when untimed leading filler coverage
///   still remains. Otherwise it is treated as stale inherited timing and reset
///   to the FA word start. Trailing coverage still never shrinks so gestures
///   and similar non-alignable material are preserved.
///
/// When FA produces no word timings at all (all `None`), the existing bullet
/// is left unchanged — the UTR hint is the only timing we have and must be
/// preserved.
///
/// # Invariant
///
/// Every bullet written by this function has `BulletSource::Authoritative`,
/// marking it as FA-derived (no longer a provisional UTR hint).
pub fn update_utterance_bullet(utterance: &mut Utterance) {
    use talkbank_model::model::BulletSource;

    const MAX_AUTHORITATIVE_START_LEAD_MS: u64 = 2_000;

    let mut first_start: Option<u64> = None;
    let mut last_end: Option<u64> = None;

    let mut timings: Vec<Option<TimeSpan>> = Vec::new();
    postprocess::collect_word_timings(&utterance.main.content.content, &mut timings);
    let has_fa_wor = utterance.wor_tier().is_some();
    let has_untimed_leading_filler_coverage =
        has_untimed_leading_filler_coverage(&utterance.main.content.content);

    for span in timings.iter().flatten() {
        if first_start.is_none_or(|s| span.start_ms < s) {
            first_start = Some(span.start_ms);
        }
        if last_end.is_none_or(|e| span.end_ms > e) {
            last_end = Some(span.end_ms);
        }
    }

    if let (Some(word_start), Some(word_end)) = (first_start, last_end) {
        let (final_start, final_end) = match &utterance.main.content.bullet {
            // Provisional UTR hint: FA word span is authoritative — overwrite.
            Some(existing) if existing.source == BulletSource::Utr => (word_start, word_end),
            // Authoritative hand-linked/FA bullet: reruns with an existing %wor
            // can preserve stale starts from a previous pass. If that lead is
            // implausibly large and there is no untimed leading filler coverage
            // left to preserve, snap the start back to the FA word span.
            // Otherwise keep the old leading coverage.
            Some(existing) => {
                let start_lead_ms = word_start.saturating_sub(existing.timing.start_ms);
                let final_start = if !has_fa_wor
                    || has_untimed_leading_filler_coverage
                    || start_lead_ms <= MAX_AUTHORITATIVE_START_LEAD_MS
                {
                    word_start.min(existing.timing.start_ms)
                } else {
                    word_start
                };
                (final_start, word_end.max(existing.timing.end_ms))
            }
            // No pre-existing bullet: set from word span.
            None => (word_start, word_end),
        };
        // The resulting bullet is authoritative (FA-derived).
        utterance.main.content.bullet = Some(Bullet::new(final_start, final_end));
    }
    // If no word timings: leave existing bullet unchanged — UNLESS it is
    // zero-duration (start >= end), which is an invalid timing that would
    // produce E362. A zero-duration bullet from a previous buggy run must be
    // cleared; no bullet is valid CHAT, an invalid bullet is not.
    if let Some(ref existing) = utterance.main.content.bullet
        && existing.timing.start_ms >= existing.timing.end_ms
    {
        utterance.main.content.bullet = None;
    }
}

fn has_untimed_leading_filler_coverage(content: &[UtteranceContent]) -> bool {
    let mut before_first_timed_word = true;
    let mut has_untimed_leading_filler = false;

    walk_words(content, Some(TierDomain::Wor), &mut |leaf| {
        if !before_first_timed_word {
            return;
        }
        let word = match leaf {
            WordItem::Word(word) => word,
            WordItem::ReplacedWord(replaced) => &replaced.word,
            WordItem::Separator(_) => return,
        };
        if get_word_timing(word).is_some() {
            before_first_timed_word = false;
            return;
        }
        if word.category == Some(WordCategory::Filler) {
            has_untimed_leading_filler = true;
        }
    });

    has_untimed_leading_filler
}

/// Collect current main-tier word timings in the exact order FA uses for
/// extraction and injection.
///
/// This is the stable timing surface for selective reuse: when an utterance has
/// already been refreshed from `%wor`, the returned vector can be stitched
/// directly into a preserved FA group without a worker roundtrip.
pub fn collect_existing_fa_word_timings(utterance: &Utterance) -> Vec<Option<WordTiming>> {
    let mut timings = Vec::new();
    walk_words(
        &utterance.main.content.content,
        None,
        &mut |leaf| match leaf {
            WordItem::Word(word) => {
                if counts_for_tier(word, TierDomain::Wor) {
                    timings.push(get_word_timing(word));
                }
            }
            WordItem::ReplacedWord(replaced) => {
                // Mirror extraction policy: the original word is the FA unit.
                // Replacement words never receive inline_bullet after injection,
                // so collecting them would always produce None for each and
                // miscount vs. collect_fa_words → collect_preserved_group_timings
                // would return None and needlessly bypass the %wor preservation path.
                if counts_for_tier(&replaced.word, TierDomain::Wor) {
                    timings.push(get_word_timing(&replaced.word));
                }
            }
            WordItem::Separator(_) => {}
        },
    );
    timings
}

// ---------------------------------------------------------------------------
// Helpers shared across submodules
// ---------------------------------------------------------------------------

/// Get a mutable reference to the nth utterance in the file.
pub(super) fn get_utterance_mut(
    chat_file: &mut talkbank_model::model::ChatFile,
    utt_idx: UtteranceIdx,
) -> Option<&mut Utterance> {
    use talkbank_model::model::Line;
    let mut current = 0;
    for line in &mut chat_file.lines {
        if let Line::Utterance(utt) = line {
            if current == utt_idx.raw() {
                return Some(utt);
            }
            current += 1;
        }
    }
    None
}

/// Get the inline timing from a word, if present.
pub(super) fn get_word_timing(word: &Word) -> Option<TimeSpan> {
    word.inline_bullet
        .as_ref()
        .map(|b| TimeSpan::new(b.timing.start_ms, b.timing.end_ms))
}

// ---------------------------------------------------------------------------
// AudioIdentity
// ---------------------------------------------------------------------------

/// Content identity for an audio file used in FA cache keys.
///
/// # Invariant
///
/// Format: `"{resolved_path}|{mtime_secs}|{file_size}"`. Fast identity
/// based on filesystem metadata (no file content hashing). Created by
/// [`AudioIdentity::from_metadata`] in the server runner.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioIdentity(String);

impl AudioIdentity {
    /// Build an identity from resolved path + filesystem metadata.
    pub fn from_metadata(path: &str, mtime_secs: u64, size: u64) -> Self {
        Self(format!("{path}|{mtime_secs}|{size}"))
    }

    /// Access the raw identity string (for display/logging).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AudioIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Cache key computation
// ---------------------------------------------------------------------------

/// Compute cache key for an FA result.
///
/// Key = `BLAKE3("{audio_identity}|{start}|{end}|{text}|{timing_flag}|{engine}")`.
pub fn cache_key(
    words: &[String],
    audio_identity: &AudioIdentity,
    start_ms: u64,
    end_ms: u64,
    timing_mode: FaTimingMode,
    engine: FaEngineType,
) -> crate::chat_ops::CacheKey {
    let text = words.join(" ");
    let timing_flag = match timing_mode {
        FaTimingMode::Continuous => "no_pauses",
        FaTimingMode::WithPauses => "pauses",
    };
    let engine_str = engine.as_str();
    let input = format!(
        "{}|{start_ms}|{end_ms}|{text}|{timing_flag}|{engine_str}",
        audio_identity.0
    );
    crate::chat_ops::CacheKey::from_content(&input)
}
