//! Utterance grouping for forced alignment time windows.

use talkbank_model::model::{ChatFile, Line};
use talkbank_model::{UtteranceIdx, WordIdx};

use super::extraction::collect_fa_words;
use super::{FaGroup, FaWord, TimeSpan};

/// Whisper CTC forced-alignment hard limit on the total number of label tokens
/// (characters) that can appear in a single FA group.
///
/// Exceeding this limit causes a Python-side `ValueError: Labels' sequence
/// length N cannot exceed the maximum allowed length of 448 tokens`. Groups
/// must be split by char count as well as by time window to stay under this.
pub const WHISPER_FA_MAX_LABEL_TOKENS: usize = 448;

/// Maximum extension (ms) into the gap after the last utterance in a group.
///
/// When an utterance bullet ends before the next utterance starts, we extend
/// the FA group's audio window into that gap so the FA engine can hear
/// trailing fillers (`&-you_know`, `&-sort_of`) that live between utterances.
/// The extension is capped at this value to avoid bleeding into the next
/// utterance's content.
const TRAILING_GAP_EXTENSION_MS: u64 = 1500;

/// Group utterances from a ChatFile into FA segments.
///
/// Groups are split when the cumulative duration exceeds `max_group_ms`.
///
/// When `total_audio_ms` is `Some`, untimed utterances get proportionally
/// estimated audio boundaries based on word count. When `None`, untimed
/// utterances are skipped (backwards-compatible behavior).
///
/// * `chat_file` - The parsed CHAT file whose utterances will be grouped.
/// * `max_group_ms` - Maximum audio window duration (in milliseconds) per
///   group. When adding an utterance would push the group past this limit,
///   a new group is started.
/// * `total_audio_ms` - Total duration of the audio file in milliseconds.
///   When `Some`, untimed utterances receive proportionally estimated
///   boundaries (with a 2-second buffer). When `None`, untimed utterances
///   are excluded from grouping.
pub fn group_utterances(
    chat_file: &ChatFile,
    max_group_ms: u64,
    total_audio_ms: Option<u64>,
) -> Vec<FaGroup> {
    let estimates = total_audio_ms.map(|total_ms| estimate_untimed_boundaries(chat_file, total_ms));

    let mut groups: Vec<FaGroup> = Vec::new();
    let mut current_words: Vec<FaWord> = Vec::new();
    let mut current_utt_indices: Vec<UtteranceIdx> = Vec::new();
    let mut current_chars: usize = 0;
    let mut seg_start: u64 = 0;
    let mut seg_end: u64 = 0;

    let mut utt_idx: usize = 0;

    let mut extracted = Vec::new();
    for line in &chat_file.lines {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };

        let utt_span = match &utt.main.content.bullet {
            Some(b) => TimeSpan::new(b.timing.start_ms, b.timing.end_ms),
            None => match &estimates {
                Some(est) if utt_idx < est.len() => est[utt_idx],
                _ => {
                    tracing::warn!(
                        utterance = utt_idx,
                        "no timing bullet and no estimate, skipping from FA grouping"
                    );
                    utt_idx += 1;
                    continue;
                }
            },
        };

        // Extract words first so we can count chars before deciding to flush.
        // (drain(..) in the loop below empties `extracted` each iteration)
        collect_fa_words(&utt.main.content.content, &mut extracted);
        let utt_chars: usize = extracted.iter().map(|w| w.len()).sum();

        // Start a new group when this utterance would push the current group past
        // either the time window or Whisper's character-token limit.
        //
        // The char-limit guard is necessary because Whisper CTC FA fails with
        // "Labels' sequence length N cannot exceed the maximum allowed length of
        // 448 tokens" when the total character count in a group exceeds 448.
        // Dense transcripts (fast speech, Spanish/long-word languages) can hit
        // this within a normal time window.
        //
        // Exception: if the current group is empty, we include the utterance
        // regardless — an utterance that alone exceeds the limit still needs to
        // be sent (and will produce a graceful Python-side error rather than
        // silently dropping the utterance).
        let over_time =
            utt_span.end_ms <= seg_start || (utt_span.end_ms - seg_start) > max_group_ms;
        let over_chars = current_chars + utt_chars > WHISPER_FA_MAX_LABEL_TOKENS;
        if !current_words.is_empty() && (over_time || over_chars) {
            // Extend the audio window into the gap before this next utterance
            // so the FA engine can hear trailing fillers at utterance boundaries.
            let extended_end = extend_into_trailing_gap(seg_end, utt_span.start_ms);
            groups.push(FaGroup {
                audio_span: TimeSpan::new(seg_start, extended_end),
                words: std::mem::take(&mut current_words),
                utterance_indices: std::mem::take(&mut current_utt_indices),
            });
            seg_start = utt_span.start_ms;
            current_chars = 0;
        }

        if current_words.is_empty() {
            seg_start = utt_span.start_ms;
        }
        seg_end = utt_span.end_ms;
        current_chars += utt_chars;

        for (word_idx, w) in extracted.drain(..).enumerate() {
            current_words.push(FaWord {
                utterance_index: UtteranceIdx(utt_idx),
                utterance_word_index: WordIdx(word_idx),
                text: w,
            });
        }

        current_utt_indices.push(UtteranceIdx(utt_idx));
        utt_idx += 1;
    }

    // Push the last group — extend into trailing audio if total duration is known.
    if !current_words.is_empty() {
        let extended_end = match total_audio_ms {
            Some(total) => extend_into_trailing_gap(seg_end, total),
            None => seg_end, // unknown audio length — don't extend blindly
        };
        groups.push(FaGroup {
            audio_span: TimeSpan::new(seg_start, extended_end),
            words: current_words,
            utterance_indices: current_utt_indices,
        });
    }

    groups
}

/// Extend an audio window's end into the gap before the next utterance.
///
/// Returns `seg_end + min(gap / 2, TRAILING_GAP_EXTENSION_MS)` — we take
/// at most half the gap to avoid bleeding into the next utterance's audio,
/// capped at the configured maximum extension.
fn extend_into_trailing_gap(seg_end: u64, next_utt_start: u64) -> u64 {
    if next_utt_start <= seg_end {
        return seg_end; // no gap (overlap or adjacent)
    }
    let gap = next_utt_start - seg_end;
    let extension = (gap / 2).min(TRAILING_GAP_EXTENSION_MS);
    seg_end + extension
}

/// Count utterances with and without timing bullets.
///
/// Returns `(timed, untimed)` — the number of utterances that have a
/// timing bullet and the number that lack one. Non-utterance lines
/// (headers, comments) are not counted.
pub fn count_utterance_timing(chat_file: &ChatFile) -> (usize, usize) {
    let (mut timed, mut untimed) = (0, 0);
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            if utt.main.content.bullet.is_some() {
                timed += 1;
            } else {
                untimed += 1;
            }
        }
    }
    (timed, untimed)
}

/// Pre-compute interpolated estimates for ALL utterances (indexed by utt_idx).
///
/// For timed utterances the estimate is unused (the real bullet is preferred).
/// For untimed utterances the estimate is interpolated from the nearest
/// neighboring timed utterances, with time distributed proportionally by
/// word count within each gap. Falls back to proportional distribution
/// across the full audio when no timed neighbors exist.
///
/// * `chat_file` - The parsed CHAT file to compute estimates for.
/// * `total_audio_ms` - Total audio duration in milliseconds. Used as
///   the boundary when untimed utterances have no timed neighbor on one side.
pub fn estimate_untimed_boundaries(chat_file: &ChatFile, total_audio_ms: u64) -> Vec<TimeSpan> {
    const BUFFER_MS: u64 = 2000;

    // Collect word counts and existing timing for each utterance.
    let mut info: Vec<(usize, Option<TimeSpan>)> = Vec::new();
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            let mut words = Vec::new();
            collect_fa_words(&utt.main.content.content, &mut words);
            let span = utt
                .main
                .content
                .bullet
                .as_ref()
                .map(|b| TimeSpan::new(b.timing.start_ms, b.timing.end_ms));
            info.push((words.len(), span));
        }
    }

    if info.is_empty() {
        return Vec::new();
    }

    let mut estimates = vec![TimeSpan::new(0, 0); info.len()];

    // Process runs of consecutive untimed utterances between timed anchors.
    // A "run" is a maximal sequence of untimed utterances.
    let mut i = 0;
    while i < info.len() {
        // Skip timed utterances — their estimates are unused.
        if let Some(span) = info[i].1 {
            estimates[i] = span;
            i += 1;
            continue;
        }

        // Found start of an untimed run. Find its end.
        let run_start = i;
        while i < info.len() && info[i].1.is_none() {
            i += 1;
        }
        let run_end = i; // exclusive

        // Determine the gap boundaries from neighboring timed utterances.
        let gap_start = if run_start > 0 {
            // Previous timed utterance's end_ms
            info[..run_start]
                .iter()
                .rev()
                .find_map(|(_, span)| span.as_ref())
                .map_or(0, |s| s.end_ms)
        } else {
            0
        };
        let gap_end = if run_end < info.len() {
            // Next timed utterance's start_ms
            info[run_end..]
                .iter()
                .find_map(|(_, span)| span.as_ref())
                .map_or(total_audio_ms, |s| s.start_ms)
        } else {
            total_audio_ms
        };

        // Distribute the gap proportionally by word count.
        let run_words: usize = info[run_start..run_end].iter().map(|(w, _)| w).sum();
        if run_words == 0 {
            // No words — give each utterance a zero-width span at gap_start.
            for est in estimates.iter_mut().take(run_end).skip(run_start) {
                *est = TimeSpan::new(gap_start, gap_start);
            }
            continue;
        }

        let gap_duration = gap_end.saturating_sub(gap_start);
        let mut words_before: usize = 0;
        for idx in run_start..run_end {
            let count = info[idx].0;
            let raw_start =
                gap_start + (words_before as f64 / run_words as f64 * gap_duration as f64) as u64;
            let raw_end = gap_start
                + ((words_before + count) as f64 / run_words as f64 * gap_duration as f64) as u64;

            let start = raw_start.saturating_sub(BUFFER_MS);
            let end = (raw_end + BUFFER_MS).min(total_audio_ms);

            estimates[idx] = TimeSpan::new(start, end);
            words_before += count;
        }
    }

    estimates
}
