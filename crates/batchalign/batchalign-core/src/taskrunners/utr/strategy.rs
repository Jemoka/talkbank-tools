//! UTR strategy core — global single-pass Hirschberg alignment over the
//! whole CHAT word stream vs. the ASR token stream.
//!
//! Ported from tbtbt's `crates/batchalign/src/chat_ops/fa/utr.rs`. The DP
//! itself comes from `talkbank_transform::dp_align`, which is the same
//! Hirschberg implementation tbtbt uses; we removed tbtbt's private copy.

use talkbank_model::model::{Bullet, ChatFile, Line, Linker, OverlapIndex};
use talkbank_model::validation::Validated;

use talkbank_transform::dp_align::{self, MatchMode};

use super::extraction::collect_fa_words;
use super::overlap_markers;
use super::two_pass::GroupingContext;

/// A single ASR token with timing — the input that all UTR strategies
/// consume. Constructed from `proto::asr::AsrSegment` words by the
/// taskrunner (after the 20ms zero-duration filter).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AsrTimingToken {
    /// Token text (single word).
    pub text: String,
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
}

/// Result summary from one UTR strategy `inject` call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UtrResult {
    /// Utterances that received timing from ASR tokens.
    pub injected: usize,
    /// Already-timed utterances (left unchanged).
    pub skipped: usize,
    /// Untimed utterances that could not be matched to ASR tokens.
    pub unmatched: usize,
    /// Per-utterance decision records for unmatched / zero-duration cases.
    /// Skipped in serialization — this is provenance, not result semantics.
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

/// Strategy trait for UTR injection. Operates on `ChatFile<Validated>`
/// because UTR only runs as part of the validated pipeline; tests that
/// want to exercise the algorithm can validate-then-strategy.
pub trait UtrStrategy: Send + Sync {
    /// Stable diagnostic label for policy tests and tracing.
    fn name(&self) -> &'static str;

    /// Inject utterance-level timing from ASR tokens into untimed CHAT utterances.
    fn inject(
        &self,
        chat_file: &mut ChatFile<Validated>,
        asr_tokens: &[AsrTimingToken],
    ) -> UtrResult;
}

/// Global single-pass UTR strategy: one flat Hirschberg alignment of all
/// CHAT words against all ASR tokens, monotonic.
pub struct GlobalUtr;

impl UtrStrategy for GlobalUtr {
    fn name(&self) -> &'static str {
        "global"
    }

    fn inject(&self, chat_file: &mut ChatFile<Validated>, asr_tokens: &[AsrTimingToken]) -> UtrResult {
        run_global_utr(chat_file, asr_tokens, false, MatchMode::CaseInsensitive)
    }
}

/// Select the default UTR strategy for a given CHAT file.
///
/// Automatic two-pass overlap recovery is intentionally disabled. The fork
/// found that its experimental pass-2 windowing could emit incorrect overlap
/// end times and now keeps `Auto` on the validated monotonic global path.
/// Retaining this selector signature avoids churn when an explicit, calibrated
/// two-pass opt-in is added later.
pub fn select_strategy(
    _chat_file: &ChatFile<Validated>,
    _grouping_context: Option<GroupingContext>,
) -> Box<dyn UtrStrategy> {
    Box::new(GlobalUtr)
}

/// Pre-extracted utterance metadata used while planning one UTR pass.
#[derive(Debug, Clone)]
pub(super) struct UtrUtteranceInfo {
    pub(super) words: Vec<String>,
    pub(super) has_bullet: bool,
    pub(super) has_lazy_overlap: bool,
    pub(super) has_ca_overlap: bool,
    pub(super) overlap_onset_fraction: Option<f64>,
    pub(super) speaker: String,
    pub(super) bottom_indices: Vec<Option<OverlapIndex>>,
    pub(super) top_onsets: Vec<(Option<OverlapIndex>, f64)>,
}

pub(super) type UtrTokenRanges = Vec<Option<(usize, usize)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UtrAlignmentStrategy {
    UniqueExactSubsequence,
    GlobalDp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UtrAlignmentPlan {
    pub(super) strategy: UtrAlignmentStrategy,
    pub(super) utt_ranges: UtrTokenRanges,
}

/// Convenience entry that runs [`GlobalUtr`].
pub fn inject_utr_timing(chat_file: &mut ChatFile<Validated>, asr_tokens: &[AsrTimingToken]) -> UtrResult {
    GlobalUtr.inject(chat_file, asr_tokens)
}

/// Core global UTR implementation. Shared between [`GlobalUtr`] and the
/// first pass of [`TwoPassOverlapUtr`].
///
/// When `skip_lazy_overlap` is true, `+<` and `⌊`-bearing utterances are
/// excluded from the flattened word sequence; pass 2 handles them.
pub(super) fn run_global_utr(
    chat_file: &mut ChatFile<Validated>,
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
        for line in &chat_file.lines.0 {
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

    let utt_line_indices: Vec<usize> = chat_file
        .lines
        .0
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

    let mut bullets_to_set: Vec<Option<(u64, u64)>> = vec![None; utt_infos.len()];
    for (utt_idx, info) in utt_infos.iter().enumerate() {
        if info.has_bullet {
            result.skipped += 1;
            continue;
        }
        match plan.utt_ranges[utt_idx] {
            Some((min_asr, max_asr)) => {
                let start_ms = asr_tokens[min_asr].start_ms;
                let end_ms = asr_tokens[max_asr].end_ms;
                if start_ms < end_ms {
                    bullets_to_set[utt_idx] = Some((start_ms, end_ms));
                    result.injected += 1;
                } else {
                    // Zero-duration span (Whisper 20ms backchannels). Skip
                    // bullet assignment — emitting `•T_T•` perpetuates
                    // through subsequent FA / align reruns. See tbtbt
                    // `chat_ops/fa/utr.rs:300-336`.
                    result.unmatched += 1;
                    if let Some(&line_idx) = utt_line_indices.get(utt_idx)
                        && let Some(Line::Utterance(utt)) = chat_file.lines.0.get(line_idx)
                    {
                        result.decisions.push(
                            talkbank_transform::decisions::DecisionRecord {
                                line_idx,
                                speaker: utt.main.speaker.as_str().to_string(),
                                strategy: talkbank_transform::decisions::DecisionStrategy::Utr(
                                    talkbank_transform::decisions::UtrStrategy::ZeroDurationSkipped,
                                ),
                                reason: format!(
                                    "words={} asr_range=[{min_asr},{max_asr}] start_ms={start_ms} end_ms={end_ms} reason=zero_or_negative_duration",
                                    info.words.len()
                                ),
                                needs_review: false,
                            },
                        );
                    }
                }
            }
            None => {
                result.unmatched += 1;
                if let Some(&line_idx) = utt_line_indices.get(utt_idx)
                    && let Some(Line::Utterance(utt)) = chat_file.lines.0.get(line_idx)
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

    // Post-pass: enforce strictly increasing start_ms for adjacent
    // non-overlap utterances. Whisper's 20ms DTW grid can collapse two
    // short consecutive words to the same boundary, which then yields a
    // zero-duration •T_T• under monotonicity enforcement. Tbtbt utr.rs:361.
    {
        let existing_timing: Vec<Option<(u64, u64)>> = chat_file
            .lines
            .0
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
            if info.has_lazy_overlap || info.has_ca_overlap {
                continue;
            }
            if info.has_bullet {
                if let Some(Some((_, end_ms))) = existing_timing.get(utt_idx) {
                    floor_end_ms = floor_end_ms.max(*end_ms);
                }
                continue;
            }
            if let Some((ref mut start_ms, ref mut end_ms)) = bullets_to_set[utt_idx] {
                if *start_ms < floor_end_ms {
                    *start_ms = floor_end_ms;
                    if *end_ms <= *start_ms {
                        *end_ms = *start_ms + 1;
                    }
                }
                floor_end_ms = *end_ms;
            }
        }
    }

    // Apply bullets to the actual ChatFile utterances.
    let mut utt_idx = 0;
    for line in &mut chat_file.lines.0 {
        if let Line::Utterance(utt) = line {
            if let Some((start_ms, end_ms)) = bullets_to_set[utt_idx] {
                utt.main.content.bullet = Some(Bullet::utr_hint(start_ms, end_ms));
            }
            utt_idx += 1;
        }
    }

    result
}

/// Extract alignable words, bullet presence, `+<` linker status, and CA
/// overlap marker info for every utterance in the order UTR sees them.
pub(super) fn collect_utr_utterance_info(chat_file: &ChatFile<Validated>) -> Vec<UtrUtteranceInfo> {
    let mut utt_infos = Vec::new();
    for line in &chat_file.lines.0 {
        if let Line::Utterance(utt) = line {
            let mut words = Vec::new();
            collect_fa_words(&utt.main.content.content.0, &mut words);
            let has_lazy_overlap = utt
                .main
                .content
                .linkers
                .0
                .contains(&Linker::LazyOverlapPrecedes);
            let overlap_info = overlap_markers::extract_overlap_info(&utt.main.content.content.0);

            let bottom_indices: Vec<_> = overlap_info
                .regions
                .iter()
                .filter(|r| {
                    r.kind == talkbank_model::alignment::helpers::OverlapRegionKind::Bottom
                        && r.has_begin()
                })
                .map(|r| r.index)
                .collect();

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

/// Plan per-utterance ASR token ranges for one UTR pass. Fast path first
/// (uniquely-embedded monotonic subsequence); Hirschberg DP fallback.
pub(super) fn plan_utr_alignment(
    all_words: &[String],
    asr_texts: &[String],
    word_to_utt: &[usize],
    utt_count: usize,
    dp_match_mode: MatchMode,
) -> UtrAlignmentPlan {
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

pub(super) fn collect_utt_ranges_from_match_indices(
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

pub(super) fn collect_utt_ranges_from_alignment(
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

pub(super) fn update_utt_range(utt_range: &mut Option<(usize, usize)>, reference_idx: usize) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Chat;
    use crate::utils::SourceId;

    fn make_tokens(words: &[(&str, u64, u64)]) -> Vec<AsrTimingToken> {
        words
            .iter()
            .map(|(t, s, e)| AsrTimingToken {
                text: t.to_string(),
                start_ms: *s,
                end_ms: *e,
            })
            .collect()
    }

    fn parse_chat(text: &str) -> Chat {
        let sid = SourceId::try_new("test.cha").unwrap();
        Chat::parse(text, sid).unwrap()
    }

    /// All-untimed CHAT + matching ASR tokens → every utterance gets a
    /// bullet projected from its first/last matched token.
    #[test]
    fn global_utr_injects_bullets_on_untimed_utterances() {
        const CHA: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world .\n*CHI:\tgoodbye now .\n@End\n";
        let mut chat = parse_chat(CHA);
        let tokens = make_tokens(&[
            ("hello", 100, 500),
            ("world", 600, 1100),
            ("goodbye", 2000, 2600),
            ("now", 2700, 3000),
        ]);
        let result = inject_utr_timing(chat.ast_mut(), &tokens);
        assert_eq!(result.injected, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.unmatched, 0);

        // Verify both utterances now carry a UTR-hint bullet.
        let mut bullets = Vec::new();
        for line in chat.ast().lines.0.iter() {
            if let Line::Utterance(u) = line {
                if let Some(b) = u.main.content.bullet.as_ref() {
                    bullets.push((b.timing.start_ms, b.timing.end_ms));
                }
            }
        }
        assert_eq!(bullets, vec![(100, 1100), (2000, 3000)]);
    }

    /// Already-timed CHAT → all utterances skipped, no bullets touched.
    #[test]
    fn global_utr_skips_already_timed_utterances() {
        const CHA: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello world . \u{15}100_1100\u{15}\n@End\n";
        let mut chat = parse_chat(CHA);
        let tokens = make_tokens(&[("hello", 100, 500), ("world", 600, 1100)]);
        let result = inject_utr_timing(chat.ast_mut(), &tokens);
        assert_eq!(result.injected, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.unmatched, 0);
    }

    /// Empty ASR token stream → every untimed utterance counted as unmatched.
    #[test]
    fn global_utr_records_unmatched_with_empty_tokens() {
        const CHA: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let mut chat = parse_chat(CHA);
        let result = inject_utr_timing(chat.ast_mut(), &[]);
        assert_eq!(result.injected, 0);
        assert_eq!(result.unmatched, 1);
    }

    #[test]
    fn default_selector_keeps_overlap_files_on_global_utr() {
        const CHA: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|||||Child|||\n@ID:\teng|corpus|MOT|||||Mother|||\n*CHI:\thello there .\n*MOT:\t+< yeah .\n@End\n";
        let chat = parse_chat(CHA);
        assert_eq!(select_strategy(chat.ast(), None).name(), "global");
    }
}
