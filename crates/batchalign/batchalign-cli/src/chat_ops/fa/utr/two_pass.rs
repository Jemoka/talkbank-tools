//! Two-pass UTR strategy for `+<` overlap-aware timing recovery.
//!
//! When overlapping speech is transcribed as separate utterances with `+<`
//! linkers, the backchannel words end up at the wrong position in the global
//! DP reference sequence. Pass 1 excludes `+<` utterances so the main-speaker
//! words align correctly, then pass 2 recovers backchannel timing from the
//! previous utterance's audio window.
//!
//! ## FA grouping stability
//!
//! Two-pass UTR can change anchor points for `estimate_untimed_boundaries`,
//! which alters FA group boundaries. Fewer, wider groups cause worse FA
//! alignment and lower final coverage — observed on German and Welsh files.
//!
//! When [`GroupingContext`] is provided, the best-of-both comparison uses FA
//! group counts as the primary signal: if two-pass creates fewer groups than
//! global, it falls back to global UTR before FA runs. Timed utterance count
//! remains the tiebreaker when grouping context is unavailable or group counts
//! are equal.

use talkbank_model::model::{Bullet, ChatFile, Line};

use crate::chat_ops::fa::grouping::group_utterances;
use talkbank_transform::dp_align::{self, MatchMode};

use super::{
    AsrTimingToken, UtrResult, UtrStrategy, UtrUtteranceInfo, collect_utr_utterance_info,
    run_global_utr,
};

/// Parameters needed to compare FA grouping outcomes between strategies.
///
/// When provided, the best-of-both fallback in [`TwoPassOverlapUtr`] uses
/// `group_utterances()` to compare how many FA groups each strategy would
/// produce. Fewer groups means wider FA windows and worse alignment — the
/// specific failure mode observed on non-English files.
#[derive(Debug, Clone, Copy)]
pub struct GroupingContext {
    /// Total audio duration in milliseconds (needed for untimed boundary
    /// estimation inside `group_utterances`).
    pub total_audio_ms: u64,
    /// Maximum FA group duration in milliseconds.
    pub max_group_ms: u64,
}

/// Whether CA overlap markers (⌈⌉⌊⌋) are used for onset windowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaMarkerPolicy {
    /// Use CA markers for onset windowing when present (default).
    #[default]
    Enabled,
    /// Ignore CA markers — treat all overlaps as `+<` only.
    Disabled,
}

impl CaMarkerPolicy {
    /// Whether CA marker processing is active.
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Word matching strategy for UTR DP alignment.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtrMatchMode {
    /// Case-insensitive exact matching (default).
    #[default]
    Exact,
    /// Fuzzy matching using Jaro-Winkler similarity.
    /// Accepts matches above the threshold (0.0–1.0).
    Fuzzy {
        /// Minimum similarity to accept (default: 0.85).
        threshold: f64,
    },
}

impl UtrMatchMode {
    /// Convert to the `dp_align::MatchMode` used by the alignment engine.
    pub(crate) fn to_dp_match_mode(self) -> MatchMode {
        match self {
            UtrMatchMode::Exact => MatchMode::CaseInsensitive,
            UtrMatchMode::Fuzzy { threshold } => MatchMode::Fuzzy { threshold },
        }
    }
}

/// Tunable parameters for the two-pass overlap-aware UTR strategy.
///
/// All fields have documented defaults that were tuned empirically on
/// SBCSAE (English), Jefferson NB (dense CA), and TaiwanHakka (Hakka).
/// Users can override individual parameters via CLI flags.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TwoPassConfig {
    /// Whether to use CA overlap markers (⌈⌉⌊⌋) for onset windowing.
    /// When enabled, the ⌈ position in the predecessor narrows the pass-2
    /// search window.
    pub ca_markers: CaMarkerPolicy,

    /// Maximum fraction of utterances that can have overlap markers before
    /// the strategy stops excluding them from pass 1. Above this threshold,
    /// excluding too many words starves the global DP of context.
    /// Default: 0.30 (30%).
    pub max_exclusion_density: f64,

    /// Tight window buffer for pass-2 recovery (milliseconds). The first
    /// attempt searches [onset ± this buffer]. Default: 500ms.
    pub tight_buffer_ms: u64,

    /// Word matching strategy for DP alignment.
    pub match_mode: UtrMatchMode,
}

impl Default for TwoPassConfig {
    /// Default configuration tuned empirically on SBCSAE (English CA),
    /// Jefferson NB (dense CA), TaiwanHakka (Hakka), and APROCSA (English
    /// aphasia). Fuzzy matching at 0.85 is the default because it provides
    /// the best coverage/precision tradeoff across all tested corpora.
    fn default() -> Self {
        Self {
            ca_markers: CaMarkerPolicy::default(),
            max_exclusion_density: 0.30,
            tight_buffer_ms: 500,
            match_mode: UtrMatchMode::Fuzzy { threshold: 0.85 },
        }
    }
}

/// Two-pass overlap-aware UTR strategy.
///
/// **Pass 1:** Runs the global alignment with overlap utterances excluded from
/// the flattened word sequence (unless overlap density exceeds
/// [`max_exclusion_density`](TwoPassConfig::max_exclusion_density)).
///
/// **Pass 2:** For each overlap utterance, finds the previous utterance's bullet
/// and runs a small DP alignment to recover timing. When CA markers are enabled
/// and the predecessor has ⌈ markers, the search window is anchored at the
/// estimated overlap onset.
///
/// ## Tunable parameters
///
/// See [`TwoPassConfig`] for all configurable parameters and their defaults.
///
/// ## FA grouping stability
///
/// When [`grouping_context`](Self::grouping_context) is set, the best-of-both
/// comparison checks FA group counts: if two-pass creates fewer groups than
/// global, it falls back to global to avoid the wider-window regression.
pub struct TwoPassOverlapUtr {
    /// Optional: total audio duration and max group size for grouping comparison.
    /// When set, the best-of-both fallback compares FA grouping outcomes.
    pub grouping_context: Option<GroupingContext>,
    /// Tunable parameters for the two-pass algorithm.
    pub config: TwoPassConfig,
}

impl Default for TwoPassOverlapUtr {
    fn default() -> Self {
        Self::new()
    }
}

impl TwoPassOverlapUtr {
    /// Create a `TwoPassOverlapUtr` with default config and no grouping context.
    pub fn new() -> Self {
        Self {
            grouping_context: None,
            config: TwoPassConfig::default(),
        }
    }

    /// Create a `TwoPassOverlapUtr` with grouping context for FA stability.
    pub fn with_grouping_context(total_audio_ms: u64, max_group_ms: u64) -> Self {
        Self {
            grouping_context: Some(GroupingContext {
                total_audio_ms,
                max_group_ms,
            }),
            config: TwoPassConfig::default(),
        }
    }

    /// Set custom configuration.
    pub fn with_config(mut self, config: TwoPassConfig) -> Self {
        self.config = config;
        self
    }
}

impl UtrStrategy for TwoPassOverlapUtr {
    fn inject(&self, chat_file: &mut ChatFile, asr_tokens: &[AsrTimingToken]) -> UtrResult {
        // Run two-pass on a clone so we can compare against global.
        let mut two_pass_file = chat_file.clone();
        let two_pass_result = run_two_pass_inner(&mut two_pass_file, asr_tokens, &self.config);

        // Run global on a separate clone for comparison.
        let mut global_file = chat_file.clone();
        let global_result = run_global_utr(
            &mut global_file,
            asr_tokens,
            false,
            self.config.match_mode.to_dp_match_mode(),
        );

        let prefer_two_pass = if let Some(ctx) = &self.grouping_context {
            // Primary signal: FA group count. Fewer groups means wider FA
            // windows, which causes worse alignment on non-English files.
            let two_pass_groups =
                group_utterances(&two_pass_file, ctx.max_group_ms, Some(ctx.total_audio_ms)).len();
            let global_groups =
                group_utterances(&global_file, ctx.max_group_ms, Some(ctx.total_audio_ms)).len();

            if two_pass_groups != global_groups {
                // Prefer whichever creates more groups (more precise FA windows).
                two_pass_groups >= global_groups
            } else {
                // Equal groups — fall back to timed utterance count.
                let two_pass_timed = count_timed_utterances(&two_pass_file);
                let global_timed = count_timed_utterances(&global_file);
                // When equal, prefer two-pass (better backchannel placement).
                two_pass_timed >= global_timed
            }
        } else {
            // No grouping context — use timed utterance count only.
            let two_pass_timed = count_timed_utterances(&two_pass_file);
            let global_timed = count_timed_utterances(&global_file);
            two_pass_timed >= global_timed
        };

        if prefer_two_pass {
            *chat_file = two_pass_file;
            two_pass_result
        } else {
            tracing::info!(
                "Two-pass UTR created fewer FA groups than global — falling back to global"
            );
            *chat_file = global_file;
            global_result
        }
    }
}

/// Core two-pass implementation: pass 1 excludes overlap utterances, pass 2
/// recovers them.
///
/// When overlap density exceeds `config.max_exclusion_density`, pass 1
/// includes all utterances in the global DP to prevent context starvation.
fn run_two_pass_inner(
    chat_file: &mut ChatFile,
    asr_tokens: &[AsrTimingToken],
    config: &TwoPassConfig,
) -> UtrResult {
    // Check overlap density to decide whether to exclude from pass 1.
    let pre_infos = collect_utr_utterance_info(chat_file);
    let total_utts = pre_infos.len();
    let overlap_utts = pre_infos
        .iter()
        .filter(|i| i.has_lazy_overlap || (config.ca_markers.is_enabled() && i.has_ca_overlap))
        .count();
    let overlap_fraction = if total_utts > 0 {
        overlap_utts as f64 / total_utts as f64
    } else {
        0.0
    };

    let skip_in_pass1 = overlap_fraction <= config.max_exclusion_density;

    if !skip_in_pass1 {
        tracing::info!(
            overlap_fraction = format!("{:.1}%", overlap_fraction * 100.0),
            overlap_utts,
            total_utts,
            "Overlap density too high for exclusion — including all in pass 1"
        );
    }

    // Pass 1: global alignment, optionally excluding overlap utterances.
    let mut result = run_global_utr(
        chat_file,
        asr_tokens,
        skip_in_pass1,
        config.match_mode.to_dp_match_mode(),
    );

    if asr_tokens.is_empty() {
        return result;
    }

    // Pass 2: recover timing for overlap utterances from predecessor windows.
    // When skip_in_pass1 is false, pass 2 only runs on utterances that
    // didn't get timing from the global DP (they participated but weren't
    // matched). The onset fraction still helps narrow recovery windows.
    let utt_infos = collect_utr_utterance_info(chat_file);

    // Collect current bullets (after pass 1) for window lookup.
    let utt_bullets: Vec<Option<(u64, u64)>> = chat_file
        .lines
        .iter()
        .filter_map(|line| {
            if let Line::Utterance(utt) = line {
                Some(
                    utt.main
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

    // Track which +< utterances we successfully time in pass 2.
    let mut pass2_bullets: Vec<(usize, u64, u64)> = Vec::new();

    for (utt_idx, info) in utt_infos.iter().enumerate() {
        // Recover timing for +< utterances and ⌊-bearing (CA overlap) utterances.
        let is_overlap =
            info.has_lazy_overlap || (config.ca_markers.is_enabled() && info.has_ca_overlap);
        if !is_overlap || info.has_bullet || info.words.is_empty() {
            continue;
        }

        // Find predecessor's overlap onset fraction (from ⌈ markers) for
        // marker-aware windowing. Only used when CA markers are enabled.
        let pred_onset_fraction = if config.ca_markers.is_enabled() {
            find_predecessor_onset_fraction(utt_idx, &utt_infos)
        } else {
            None
        };

        // Adaptive window: try narrow first, widen on failure.
        if let Some((start_ms, end_ms)) = recover_with_adaptive_window(
            &info.words,
            asr_tokens,
            utt_idx,
            &utt_bullets,
            pred_onset_fraction,
            config,
        ) {
            pass2_bullets.push((utt_idx, start_ms, end_ms));
            // Adjust counts: this was counted as unmatched in pass 1.
            result.unmatched -= 1;
            result.injected += 1;
        }
    }

    // Apply pass 2 bullets to the ChatFile.
    if !pass2_bullets.is_empty() {
        let mut utt_idx = 0;
        let mut bullet_iter = pass2_bullets.iter().peekable();
        for line in &mut chat_file.lines {
            if let Line::Utterance(utt) = line {
                if let Some(&&(target_idx, start_ms, end_ms)) = bullet_iter.peek()
                    && utt_idx == target_idx
                {
                    utt.main.content.bullet = Some(Bullet::new(start_ms, end_ms));
                    bullet_iter.next();
                }
                utt_idx += 1;
            }
        }
    }

    result
}

/// Count utterances that have a bullet (timed) in the chat file.
fn count_timed_utterances(chat_file: &ChatFile) -> usize {
    chat_file
        .lines
        .iter()
        .filter(|line| {
            if let Line::Utterance(utt) = line {
                utt.main.content.bullet.is_some()
            } else {
                false
            }
        })
        .count()
}

/// Recover backchannel timing with an adaptive window strategy.
///
/// Instead of a fixed buffer, tries increasingly wider windows around the
/// predecessor utterance until a match is found or all attempts are exhausted.
///
/// **Strategy:**
/// 1. **Narrow (±500ms):** Tight window around predecessor. Works well when
///    ASR timing is accurate (typical for English).
/// 2. **Predecessor duration:** Window equals the predecessor's full duration
///    added as buffer on each side. Catches backchannels whose ASR timing
///    drifts by up to the utterance length.
/// 3. **Double predecessor duration:** For poor ASR (non-English) where timing
///    can be significantly offset.
///
/// **CA overlap marker optimization:** When `pred_onset_fraction` is provided
/// (from ⌈ markers on the predecessor), the search window is anchored at the
/// estimated overlap onset point instead of the full predecessor start. This
/// is typically a much tighter window, helping especially when ASR quality is
/// poor on non-English data.
///
/// Stops at the first match to prefer the tightest plausible window.
fn recover_with_adaptive_window(
    words: &[String],
    asr_tokens: &[AsrTimingToken],
    utt_idx: usize,
    utt_bullets: &[Option<(u64, u64)>],
    pred_onset_fraction: Option<f64>,
    config: &TwoPassConfig,
) -> Option<(u64, u64)> {
    // Find predecessor bullet
    let (pred_start, pred_end) = find_predecessor_bullet(utt_idx, utt_bullets)?;
    let pred_duration = pred_end.saturating_sub(pred_start);

    // If the predecessor has ⌈ markers, compute the estimated overlap onset
    // time. This anchors the search window at the point where overlap begins
    // rather than the full predecessor start.
    let anchor_start = match pred_onset_fraction {
        Some(fraction) => {
            let onset = pred_start + (fraction * pred_duration as f64) as u64;
            tracing::debug!(
                onset_ms = onset,
                fraction,
                pred_start,
                pred_end,
                "CA overlap marker: anchoring search at estimated onset"
            );
            onset
        }
        None => pred_start,
    };

    // Try increasingly wider buffers around the anchor point
    let buffers = [
        config.tight_buffer_ms,        // tight: ±configured buffer
        pred_duration.max(2000),       // medium: ±predecessor duration (min 2s)
        (pred_duration * 2).max(5000), // wide: ±2x predecessor duration (min 5s)
    ];

    for buffer_ms in buffers {
        let window_start = anchor_start.saturating_sub(buffer_ms);
        let window_end = pred_end + buffer_ms;

        if let Some(timing) = recover_overlap_timing(
            words,
            asr_tokens,
            window_start,
            window_end,
            config.match_mode.to_dp_match_mode(),
        ) {
            return Some(timing);
        }
    }

    None
}

/// Find the overlap onset fraction from the nearest preceding utterance
/// whose top overlap marker matches the current utterance's bottom index.
///
/// Searches backward from `utt_idx` for a top region on a *different*
/// speaker whose index matches one of the current utterance's bottom
/// indices. This enables 1:N matching — multiple bottom utterances from
/// different speakers all get the same onset fraction from the shared top.
fn find_predecessor_onset_fraction(utt_idx: usize, utt_infos: &[UtrUtteranceInfo]) -> Option<f64> {
    let current = &utt_infos[utt_idx];
    let current_speaker = &current.speaker;

    // For +< utterances without CA markers, fall back to any predecessor top.
    let bottom_indices = &current.bottom_indices;

    for prev_idx in (0..utt_idx).rev() {
        let prev = &utt_infos[prev_idx];

        // Must be a different speaker.
        if prev.speaker == *current_speaker {
            continue;
        }

        // Match by index: find a top region on the predecessor whose index
        // matches one of our bottom indices.
        for (top_index, fraction) in &prev.top_onsets {
            if bottom_indices.is_empty() {
                // +< utterance without CA markers — accept any top.
                return Some(*fraction);
            }
            if bottom_indices.contains(top_index) {
                return Some(*fraction);
            }
        }

        // Stop after first different-speaker utterance with a bullet
        // (don't search too far back).
        if prev.has_bullet {
            break;
        }
    }

    // Fallback: any predecessor with an onset fraction (for +< without CA).
    if bottom_indices.is_empty() {
        for prev_idx in (0..utt_idx).rev() {
            if let Some(fraction) = utt_infos[prev_idx].overlap_onset_fraction {
                return Some(fraction);
            }
            if utt_infos[prev_idx].has_bullet {
                break;
            }
        }
    }

    None
}

/// Find the nearest preceding utterance's bullet range (no buffer applied).
fn find_predecessor_bullet(
    utt_idx: usize,
    utt_bullets: &[Option<(u64, u64)>],
) -> Option<(u64, u64)> {
    for prev_idx in (0..utt_idx).rev() {
        if let Some(bullet) = utt_bullets[prev_idx] {
            return Some(bullet);
        }
    }
    None
}

/// Recover timing for a small set of overlap words from ASR tokens within a
/// constrained time window.
///
/// Filters `asr_tokens` to those overlapping `[window_start_ms, window_end_ms]`,
/// then runs a Hirschberg DP alignment of the `+<` utterance's words against
/// the windowed tokens. Returns the matched time span if any words matched.
///
/// This is cheap — typically 1–3 backchannel words against 5–20 windowed tokens.
pub fn recover_overlap_timing(
    words: &[String],
    asr_tokens: &[AsrTimingToken],
    window_start_ms: u64,
    window_end_ms: u64,
    dp_match_mode: MatchMode,
) -> Option<(u64, u64)> {
    // Filter ASR tokens to those overlapping the window.
    let windowed: Vec<(usize, &AsrTimingToken)> = asr_tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.start_ms < window_end_ms && t.end_ms > window_start_ms)
        .collect();

    if windowed.is_empty() {
        return None;
    }

    let windowed_texts: Vec<String> = windowed.iter().map(|(_, t)| t.text.clone()).collect();

    let alignment = dp_align::align(words, &windowed_texts, dp_match_mode);

    let mut min_start: Option<u64> = None;
    let mut max_end: Option<u64> = None;

    for result_item in &alignment {
        if let dp_align::AlignResult::Match { reference_idx, .. } = result_item {
            let token = windowed[*reference_idx].1;
            match min_start {
                Some(s) if token.start_ms < s => min_start = Some(token.start_ms),
                None => min_start = Some(token.start_ms),
                _ => {}
            }
            match max_end {
                Some(e) if token.end_ms > e => max_end = Some(token.end_ms),
                None => max_end = Some(token.end_ms),
                _ => {}
            }
        }
    }

    match (min_start, max_end) {
        (Some(start), Some(end)) => Some((start, end)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_parser::TreeSitterParser;

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

    #[test]
    fn test_recover_overlap_timing_finds_mhm_in_window() {
        let words = vec!["mhm".to_string()];
        let tokens = make_asr_tokens(&[("mhm", 1800, 2200)]);
        let result = recover_overlap_timing(&words, &tokens, 0, 3000, MatchMode::CaseInsensitive);
        assert_eq!(result, Some((1800, 2200)));
    }

    #[test]
    fn test_recover_overlap_timing_no_match_outside_window() {
        let words = vec!["mhm".to_string()];
        let tokens = make_asr_tokens(&[("mhm", 5000, 5500)]);
        let result = recover_overlap_timing(&words, &tokens, 0, 3000, MatchMode::CaseInsensitive);
        assert_eq!(result, None);
    }

    #[test]
    fn test_recover_overlap_timing_multi_word() {
        let words = vec!["oh".to_string(), "okay".to_string()];
        let tokens = make_asr_tokens(&[("oh", 1500, 1700), ("okay", 1800, 2200)]);
        let result = recover_overlap_timing(&words, &tokens, 0, 3000, MatchMode::CaseInsensitive);
        assert_eq!(result, Some((1500, 2200)));
    }

    #[test]
    fn test_recover_overlap_timing_empty_window() {
        let words = vec!["mhm".to_string()];
        let tokens = make_asr_tokens(&[("mhm", 1800, 2200)]);
        // Window that doesn't overlap any tokens
        let result =
            recover_overlap_timing(&words, &tokens, 5000, 6000, MatchMode::CaseInsensitive);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_predecessor_bullet_immediate() {
        let bullets = vec![
            Some((1000, 3000)),
            None, // +< utterance at index 1
        ];
        let bullet = find_predecessor_bullet(1, &bullets);
        assert_eq!(bullet, Some((1000, 3000)));
    }

    #[test]
    fn test_find_predecessor_bullet_skips_untimed() {
        let bullets = vec![
            Some((1000, 3000)),
            None, // untimed (not +<)
            None, // +< utterance at index 2
        ];
        let bullet = find_predecessor_bullet(2, &bullets);
        assert_eq!(bullet, Some((1000, 3000)));
    }

    #[test]
    fn test_find_predecessor_bullet_none_at_start() {
        let bullets = vec![
            None, // +< utterance at index 0 — no predecessor
        ];
        let bullet = find_predecessor_bullet(0, &bullets);
        assert_eq!(bullet, None);
    }

    #[test]
    fn test_adaptive_window_finds_with_narrow() {
        let words = vec!["mhm".to_string()];
        let tokens = make_asr_tokens(&[("mhm", 1800, 2200)]);
        let bullets = vec![
            Some((1000, 3000)), // predecessor
            None,               // +< utterance
        ];
        // "mhm" at 1800 is within narrow window (1000-500=500, 3000+500=3500)
        let result = recover_with_adaptive_window(
            &words,
            &tokens,
            1,
            &bullets,
            None,
            &TwoPassConfig::default(),
        );
        assert_eq!(result, Some((1800, 2200)));
    }

    #[test]
    fn test_adaptive_window_widens_to_find_match() {
        let words = vec!["mhm".to_string()];
        // "mhm" is 5 seconds after predecessor ends — too far for narrow (±500ms)
        // but within medium (predecessor duration = 2000ms, so ±2000ms → window 0..7000)
        let tokens = make_asr_tokens(&[("mhm", 6500, 6800)]);
        let bullets = vec![
            Some((1000, 3000)), // predecessor: 2s duration
            None,               // +< utterance
        ];
        let result = recover_with_adaptive_window(
            &words,
            &tokens,
            1,
            &bullets,
            None,
            &TwoPassConfig::default(),
        );
        assert_eq!(result, Some((6500, 6800)));
    }

    #[test]
    fn test_adaptive_window_no_predecessor() {
        let words = vec!["mhm".to_string()];
        let tokens = make_asr_tokens(&[("mhm", 1800, 2200)]);
        let bullets = vec![None]; // no predecessor
        let result = recover_with_adaptive_window(
            &words,
            &tokens,
            0,
            &bullets,
            None,
            &TwoPassConfig::default(),
        );
        assert_eq!(result, None);
    }

    /// When the predecessor has ⌈ markers (overlap onset fraction), the search
    /// window anchors at the estimated onset point, enabling a tighter match.
    #[test]
    fn test_adaptive_window_with_ca_onset_fraction() {
        let words = vec!["yeah".to_string()];
        // Predecessor spans 10000..15000 (5s duration).
        // ⌈ at fraction 0.6 → onset at 13000ms.
        // "yeah" at 13200ms should be found with the tight ±500ms window
        // around the onset (12500..15500).
        let tokens = make_asr_tokens(&[("yeah", 13200, 13500)]);
        let bullets = vec![
            Some((10000, 15000)), // predecessor
            None,                 // overlap utterance
        ];
        let result = recover_with_adaptive_window(
            &words,
            &tokens,
            1,
            &bullets,
            Some(0.6),
            &TwoPassConfig::default(),
        );
        assert_eq!(result, Some((13200, 13500)));
    }

    /// Without onset fraction, a token near the end of a long predecessor
    /// would still be found (but by the wider window). With onset fraction,
    /// the tight window finds it immediately.
    #[test]
    fn test_adaptive_window_onset_fraction_narrows_window() {
        let words = vec!["mhm".to_string()];
        // Predecessor spans 0..20000 (20s duration).
        // Without onset fraction: tight window ±500ms around 0..20000 would
        // search 0..20500 and find "mhm" at 16000ms.
        // With onset fraction 0.8: tight window ±500ms around 16000..20000
        // would search 15500..20500 and find it.
        let tokens = make_asr_tokens(&[("mhm", 16000, 16300)]);
        let bullets = vec![
            Some((0, 20000)), // predecessor: 20s
            None,             // overlap utterance
        ];

        // Both should find the token
        let without = recover_with_adaptive_window(
            &words,
            &tokens,
            1,
            &bullets,
            None,
            &TwoPassConfig::default(),
        );
        let with = recover_with_adaptive_window(
            &words,
            &tokens,
            1,
            &bullets,
            Some(0.8),
            &TwoPassConfig::default(),
        );
        assert!(without.is_some(), "should find without onset fraction too");
        assert_eq!(with, Some((16000, 16300)));
    }

    /// When pass 2 leaves more unmatched than global would, the best-of-both
    /// fallback should use global results instead.
    #[test]
    fn test_best_of_both_falls_back_to_global() {
        use talkbank_parser::TreeSitterParser;

        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant, INV Investigator
@ID:\teng|test|PAR|||||Participant|||
@ID:\teng|test|INV|||||Investigator|||
@Media:\ttest, audio
*PAR:\tI went to the store yesterday .
*INV:\t+< ja .
*PAR:\tand bought some groceries .
@End
";
        let parser = TreeSitterParser::new().unwrap();
        let mut chat = parser.parse_chat_file(chat_text).unwrap();

        // ASR tokens: PAR's words + "ja" appears far from predecessor window.
        // The global DP can match "ja" because it sees the full stream.
        // Pass 2 windowed recovery can't find "ja" near the predecessor.
        let tokens = vec![
            AsrTimingToken {
                text: "I".into(),
                start_ms: 100,
                end_ms: 300,
            },
            AsrTimingToken {
                text: "went".into(),
                start_ms: 400,
                end_ms: 800,
            },
            AsrTimingToken {
                text: "to".into(),
                start_ms: 900,
                end_ms: 1100,
            },
            AsrTimingToken {
                text: "the".into(),
                start_ms: 1200,
                end_ms: 1400,
            },
            AsrTimingToken {
                text: "store".into(),
                start_ms: 1500,
                end_ms: 2000,
            },
            AsrTimingToken {
                text: "yesterday".into(),
                start_ms: 2300,
                end_ms: 3000,
            },
            // "ja" appears 50 seconds later (simulating poor ASR timing for non-English)
            AsrTimingToken {
                text: "ja".into(),
                start_ms: 50000,
                end_ms: 50500,
            },
            AsrTimingToken {
                text: "and".into(),
                start_ms: 5000,
                end_ms: 5300,
            },
            AsrTimingToken {
                text: "bought".into(),
                start_ms: 5400,
                end_ms: 5800,
            },
            AsrTimingToken {
                text: "some".into(),
                start_ms: 5900,
                end_ms: 6200,
            },
            AsrTimingToken {
                text: "groceries".into(),
                start_ms: 6300,
                end_ms: 7000,
            },
        ];

        let result = TwoPassOverlapUtr::new().inject(&mut chat, &tokens);

        // The fallback should have kicked in: global can match "ja" at 50000ms,
        // while pass 2 can't find "ja" in the predecessor window (100-3000ms ± buffer).
        // With best-of-both, all 3 utterances should be timed.
        println!(
            "injected={} skipped={} unmatched={}",
            result.injected, result.skipped, result.unmatched
        );
        assert_eq!(
            result.unmatched, 0,
            "best-of-both should fall back to global and time all utterances"
        );
        assert_eq!(result.injected, 3);
    }

    /// When grouping context is provided and two-pass creates fewer FA groups
    /// than global (the wider-window regression), the fallback should use
    /// global results even if two-pass timed more utterances.
    #[test]
    fn test_grouping_fallback_prefers_more_groups() {
        // Construct a scenario where two-pass changes bullet placement enough
        // to merge FA groups. We use a file with a +< backchannel between two
        // main-speaker utterances. The ASR tokens are positioned so two-pass
        // recovers the +< at a different time window than global would.
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant, INV Investigator
@ID:\teng|test|PAR|||||Participant|||
@ID:\teng|test|INV|||||Investigator|||
@Media:\ttest, audio
*PAR:\tI went to the store yesterday .
*INV:\t+< ja .
*PAR:\tand bought some groceries .
@End
";
        let parser = TreeSitterParser::new().unwrap();
        let mut chat = parser.parse_chat_file(chat_text).unwrap();

        // ASR tokens: "ja" far from predecessor → two-pass can't find it,
        // global can. This means two-pass leaves "ja" untimed, which changes
        // estimated boundaries and could create fewer groups.
        let tokens = vec![
            AsrTimingToken {
                text: "I".into(),
                start_ms: 100,
                end_ms: 300,
            },
            AsrTimingToken {
                text: "went".into(),
                start_ms: 400,
                end_ms: 800,
            },
            AsrTimingToken {
                text: "to".into(),
                start_ms: 900,
                end_ms: 1100,
            },
            AsrTimingToken {
                text: "the".into(),
                start_ms: 1200,
                end_ms: 1400,
            },
            AsrTimingToken {
                text: "store".into(),
                start_ms: 1500,
                end_ms: 2000,
            },
            AsrTimingToken {
                text: "yesterday".into(),
                start_ms: 2300,
                end_ms: 3000,
            },
            // "ja" at 50s — too far for two-pass windowed recovery
            AsrTimingToken {
                text: "ja".into(),
                start_ms: 50000,
                end_ms: 50500,
            },
            AsrTimingToken {
                text: "and".into(),
                start_ms: 5000,
                end_ms: 5300,
            },
            AsrTimingToken {
                text: "bought".into(),
                start_ms: 5400,
                end_ms: 5800,
            },
            AsrTimingToken {
                text: "some".into(),
                start_ms: 5900,
                end_ms: 6200,
            },
            AsrTimingToken {
                text: "groceries".into(),
                start_ms: 6300,
                end_ms: 7000,
            },
        ];

        // With grouping context: total_audio_ms covers the full range,
        // max_group_ms is small enough to create multiple groups when
        // all utterances are timed.
        let strategy = TwoPassOverlapUtr::with_grouping_context(60000, 15000);
        let result = strategy.inject(&mut chat, &tokens);

        // Global should be preferred because it can time "ja" (creating
        // tighter groups), while two-pass leaves it untimed (wider groups).
        assert_eq!(
            result.unmatched, 0,
            "grouping fallback should choose global which times all utterances"
        );
        assert_eq!(result.injected, 3);
    }

    /// When two-pass creates equal or more FA groups than global, two-pass
    /// should be preferred (better backchannel placement).
    #[test]
    fn test_grouping_keeps_two_pass_when_groups_equal_or_better() {
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant, INV Investigator
@ID:\teng|test|PAR|||||Participant|||
@ID:\teng|test|INV|||||Investigator|||
@Media:\ttest, audio
*PAR:\tI went to the store yesterday .
*INV:\t+< mhm .
*PAR:\tand bought some groceries .
@End
";
        let parser = TreeSitterParser::new().unwrap();
        let mut chat = parser.parse_chat_file(chat_text).unwrap();

        // "mhm" close to predecessor → two-pass windowed recovery succeeds.
        // Both strategies should create the same groups.
        let tokens = vec![
            AsrTimingToken {
                text: "I".into(),
                start_ms: 100,
                end_ms: 300,
            },
            AsrTimingToken {
                text: "went".into(),
                start_ms: 400,
                end_ms: 800,
            },
            AsrTimingToken {
                text: "to".into(),
                start_ms: 900,
                end_ms: 1100,
            },
            AsrTimingToken {
                text: "the".into(),
                start_ms: 1200,
                end_ms: 1400,
            },
            AsrTimingToken {
                text: "store".into(),
                start_ms: 1500,
                end_ms: 2000,
            },
            AsrTimingToken {
                text: "yesterday".into(),
                start_ms: 2300,
                end_ms: 3000,
            },
            AsrTimingToken {
                text: "mhm".into(),
                start_ms: 1800,
                end_ms: 2200,
            },
            AsrTimingToken {
                text: "and".into(),
                start_ms: 5000,
                end_ms: 5300,
            },
            AsrTimingToken {
                text: "bought".into(),
                start_ms: 5400,
                end_ms: 5800,
            },
            AsrTimingToken {
                text: "some".into(),
                start_ms: 5900,
                end_ms: 6200,
            },
            AsrTimingToken {
                text: "groceries".into(),
                start_ms: 6300,
                end_ms: 7000,
            },
        ];

        let strategy = TwoPassOverlapUtr::with_grouping_context(60000, 15000);
        let result = strategy.inject(&mut chat, &tokens);

        // Two-pass should be kept (groups equal, better backchannel placement).
        assert_eq!(result.injected, 3, "two-pass should time all 3 utterances");
        assert_eq!(result.unmatched, 0);
    }
}
