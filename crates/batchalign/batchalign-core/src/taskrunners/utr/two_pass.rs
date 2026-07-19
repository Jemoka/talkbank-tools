//! Two-pass UTR strategy for `+<` overlap-aware timing recovery.
//!
//! Pass 1 excludes overlap utterances (`+<` linkers and `⌊`-bearing CA
//! markers) from the global DP so main-speaker words align correctly.
//! Pass 2 recovers backchannel timing for those skipped utterances from
//! the previous utterance's audio window.
//!
//! Ported from `tbtbt/crates/batchalign/src/chat_ops/fa/utr/two_pass.rs`,
//! using the shared `talkbank_transform::dp_align`. The FA-grouping check
//! in tbtbt's two-pass-vs-global tiebreaker is not yet ported (depends
//! on a `group_utterances` helper that lives in tbtbt's chat_ops); the
//! `GroupingContext` field is retained as a forward-extension hook but
//! the comparison currently always falls back to the timed-utterance
//! heuristic.

use talkbank_model::model::{Bullet, ChatFile, Line};
use talkbank_model::validation::Validated;

use talkbank_transform::dp_align::{self, MatchMode};

use super::strategy::{
    AsrTimingToken, UtrResult, UtrStrategy, UtrUtteranceInfo, collect_utr_utterance_info,
    run_global_utr,
};

/// Forward-extension hook for FA-grouping-aware two-pass tiebreaking.
/// When supplied, future versions of this module will compare two-pass vs
/// global by their FA group counts (fewer groups = wider FA windows =
/// worse alignment on non-English files). Today this is unused; the
/// tiebreaker is purely the timed-utterance count.
#[derive(Debug, Clone, Copy)]
pub struct GroupingContext {
    /// Total audio duration in milliseconds.
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
    /// Case-insensitive exact matching.
    #[default]
    Exact,
    /// Fuzzy matching using Jaro-Winkler similarity.
    Fuzzy {
        /// Minimum similarity to accept (default: 0.85).
        threshold: f64,
    },
}

impl UtrMatchMode {
    pub(crate) fn to_dp_match_mode(self) -> MatchMode {
        match self {
            UtrMatchMode::Exact => MatchMode::CaseInsensitive,
            UtrMatchMode::Fuzzy { threshold } => MatchMode::Fuzzy { threshold },
        }
    }
}

/// Tunable parameters for the two-pass overlap-aware UTR strategy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TwoPassConfig {
    /// Whether to use CA overlap markers (⌈⌉⌊⌋) for onset windowing.
    pub ca_markers: CaMarkerPolicy,
    /// Maximum fraction of utterances that can have overlap markers
    /// before pass 1 stops excluding them. Default: 0.30.
    pub max_exclusion_density: f64,
    /// Tight window buffer for pass-2 recovery (milliseconds).
    /// Default: 500ms.
    pub tight_buffer_ms: u64,
    /// Word matching strategy for DP alignment.
    pub match_mode: UtrMatchMode,
}

impl Default for TwoPassConfig {
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
pub struct TwoPassOverlapUtr {
    /// Forward-extension hook for FA-grouping-aware tiebreaking.
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
    pub fn new() -> Self {
        Self {
            grouping_context: None,
            config: TwoPassConfig::default(),
        }
    }

    pub fn with_grouping_context(total_audio_ms: u64, max_group_ms: u64) -> Self {
        Self {
            grouping_context: Some(GroupingContext {
                total_audio_ms,
                max_group_ms,
            }),
            config: TwoPassConfig::default(),
        }
    }

    pub fn with_config(mut self, config: TwoPassConfig) -> Self {
        self.config = config;
        self
    }
}

impl UtrStrategy for TwoPassOverlapUtr {
    fn name(&self) -> &'static str {
        "two_pass"
    }

    fn inject(&self, chat_file: &mut ChatFile<Validated>, asr_tokens: &[AsrTimingToken]) -> UtrResult {
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

        // Tiebreaker: timed-utterance count. When equal, prefer two-pass.
        let two_pass_timed = count_timed_utterances(&two_pass_file);
        let global_timed = count_timed_utterances(&global_file);
        let prefer_two_pass = two_pass_timed >= global_timed;

        if prefer_two_pass {
            *chat_file = two_pass_file;
            two_pass_result
        } else {
            tracing::info!(
                "Two-pass UTR yielded fewer timed utterances than global — falling back to global"
            );
            *chat_file = global_file;
            global_result
        }
    }
}

/// Core two-pass implementation: pass 1 excludes overlap utterances,
/// pass 2 recovers them from predecessor windows.
fn run_two_pass_inner(
    chat_file: &mut ChatFile<Validated>,
    asr_tokens: &[AsrTimingToken],
    config: &TwoPassConfig,
) -> UtrResult {
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
    let utt_infos = collect_utr_utterance_info(chat_file);
    let utt_bullets: Vec<Option<(u64, u64)>> = chat_file
        .lines
        .0
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

    let mut pass2_bullets: Vec<(usize, u64, u64)> = Vec::new();
    for (utt_idx, info) in utt_infos.iter().enumerate() {
        let is_overlap =
            info.has_lazy_overlap || (config.ca_markers.is_enabled() && info.has_ca_overlap);
        if !is_overlap || info.has_bullet || info.words.is_empty() {
            continue;
        }
        let pred_onset_fraction = if config.ca_markers.is_enabled() {
            find_predecessor_onset_fraction(utt_idx, &utt_infos)
        } else {
            None
        };
        if let Some((start_ms, end_ms)) = recover_with_adaptive_window(
            &info.words,
            asr_tokens,
            utt_idx,
            &utt_bullets,
            pred_onset_fraction,
            config,
        ) {
            pass2_bullets.push((utt_idx, start_ms, end_ms));
            result.unmatched = result.unmatched.saturating_sub(1);
            result.injected += 1;
        }
    }

    // Apply pass 2 bullets to the ChatFile.
    if !pass2_bullets.is_empty() {
        let mut utt_idx = 0;
        let mut bullet_iter = pass2_bullets.iter().peekable();
        for line in &mut chat_file.lines.0 {
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

fn count_timed_utterances(chat_file: &ChatFile<Validated>) -> usize {
    chat_file
        .lines
        .0
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

/// Recover backchannel timing with an adaptive window strategy: tight
/// first, widen on failure. Matches tbtbt two_pass.rs:406.
fn recover_with_adaptive_window(
    words: &[String],
    asr_tokens: &[AsrTimingToken],
    utt_idx: usize,
    utt_bullets: &[Option<(u64, u64)>],
    pred_onset_fraction: Option<f64>,
    config: &TwoPassConfig,
) -> Option<(u64, u64)> {
    let (pred_start, pred_end) = find_predecessor_bullet(utt_idx, utt_bullets)?;
    let pred_duration = pred_end.saturating_sub(pred_start);
    let anchor_start = match pred_onset_fraction {
        Some(fraction) => pred_start + (fraction * pred_duration as f64) as u64,
        None => pred_start,
    };

    let buffers = [
        config.tight_buffer_ms,
        pred_duration.max(2000),
        (pred_duration * 2).max(5000),
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
/// whose top region matches the current utterance's bottom index.
fn find_predecessor_onset_fraction(utt_idx: usize, utt_infos: &[UtrUtteranceInfo]) -> Option<f64> {
    let current = &utt_infos[utt_idx];
    let current_speaker = &current.speaker;
    let bottom_indices = &current.bottom_indices;

    for prev_idx in (0..utt_idx).rev() {
        let prev = &utt_infos[prev_idx];
        if prev.speaker == *current_speaker {
            continue;
        }
        for (top_index, fraction) in &prev.top_onsets {
            if bottom_indices.is_empty() {
                return Some(*fraction);
            }
            if bottom_indices.contains(top_index) {
                return Some(*fraction);
            }
        }
        if prev.has_bullet {
            break;
        }
    }

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

/// Hirschberg DP alignment of `words` against ASR tokens windowed to a
/// `[window_start_ms, window_end_ms]` time range.
pub fn recover_overlap_timing(
    words: &[String],
    asr_tokens: &[AsrTimingToken],
    window_start_ms: u64,
    window_end_ms: u64,
    dp_match_mode: MatchMode,
) -> Option<(u64, u64)> {
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
