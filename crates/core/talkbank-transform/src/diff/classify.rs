//! DP alignment → delta classification.
//!
//! Takes the raw DP alignment between before/after utterance fingerprints
//! and classifies each pair into an [`UtteranceDelta`].

use talkbank_model::UtteranceIdx;
use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::model::{ChatFile, Line};

use crate::dp_align::{AlignResult, MatchMode, align};
use crate::extract::{self, ExtractedUtterance};

use super::types::UtteranceDelta;

/// Extract a fingerprint string for an utterance's words (Mor domain).
///
/// The fingerprint is the space-joined cleaned text of all Mor-alignable words.
/// Two utterances with the same fingerprint have identical NLP-relevant content.
fn utterance_fingerprint(extracted: &ExtractedUtterance) -> String {
    extracted
        .words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract the speaker code from an utterance at the given index.
fn speaker_at(chat_file: &ChatFile, utt_idx: UtteranceIdx) -> Option<&str> {
    let mut idx = 0usize;
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            if idx == utt_idx.raw() {
                return Some(utt.main.speaker.as_str());
            }
            idx += 1;
        }
    }
    None
}

/// Check whether the utterance-level bullet timing differs between two files.
///
/// Returns `true` if the timing changed (or one has a bullet and the other doesn't).
fn timing_differs(
    before: &ChatFile,
    before_idx: UtteranceIdx,
    after: &ChatFile,
    after_idx: UtteranceIdx,
) -> bool {
    let before_bullet = bullet_at(before, before_idx);
    let after_bullet = bullet_at(after, after_idx);
    before_bullet != after_bullet
}

/// Extract the bullet timing (start_ms, end_ms) for an utterance, if present.
fn bullet_at(chat_file: &ChatFile, utt_idx: UtteranceIdx) -> Option<(u64, u64)> {
    let mut idx = 0usize;
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            if idx == utt_idx.raw() {
                return utt
                    .main
                    .content
                    .bullet
                    .as_ref()
                    .map(|b| (b.timing.start_ms, b.timing.end_ms));
            }
            idx += 1;
        }
    }
    None
}

/// Classify a pair of utterances with identical word fingerprints.
///
/// Checks for timing-only or speaker-only changes.
fn classify_same_words(
    before: &ChatFile,
    before_idx: UtteranceIdx,
    after: &ChatFile,
    after_idx: UtteranceIdx,
) -> UtteranceDelta {
    let speaker_same = speaker_at(before, before_idx) == speaker_at(after, after_idx);
    let timing_same = !timing_differs(before, before_idx, after, after_idx);

    match (speaker_same, timing_same) {
        (true, true) => UtteranceDelta::Unchanged {
            before_idx,
            after_idx,
        },
        (true, false) => UtteranceDelta::TimingOnly {
            before_idx,
            after_idx,
        },
        (false, _) => UtteranceDelta::SpeakerChanged {
            before_idx,
            after_idx,
        },
    }
}

/// Compute utterance-level deltas between two versions of a CHAT file.
///
/// Uses DP alignment on per-utterance word fingerprints to find the best
/// correspondence between before and after utterances, then classifies each
/// pair based on word content, speaker, and timing changes.
///
/// # Algorithm
///
/// 1. Extract Mor-domain words per utterance from both files.
/// 2. Compute fingerprints (space-joined cleaned words) per utterance.
/// 3. Run Hirschberg DP alignment on the fingerprint sequences.
/// 4. Post-process alignment to detect substitution pairs
///    (adjacent ExtraPayload + ExtraReference from the DP aligner).
/// 5. Classify each result into an [`UtteranceDelta`].
pub fn diff_chat(before: &ChatFile, after: &ChatFile) -> Vec<UtteranceDelta> {
    let before_utts = extract::extract_words(before, TierDomain::Mor);
    let after_utts = extract::extract_words(after, TierDomain::Mor);

    let before_fps: Vec<String> = before_utts.iter().map(utterance_fingerprint).collect();
    let after_fps: Vec<String> = after_utts.iter().map(utterance_fingerprint).collect();

    // DP alignment: "after" is the payload (what we're producing),
    // "before" is the reference (what we're comparing against).
    let alignment = align(&after_fps, &before_fps, MatchMode::Exact);

    // Convert raw alignment results into classified deltas.
    // The DP aligner emits substitutions as adjacent ExtraPayload + ExtraReference
    // pairs. We detect these and classify them as WordsChanged.
    let mut deltas = Vec::new();
    let mut i = 0;
    while i < alignment.len() {
        match &alignment[i] {
            AlignResult::Match {
                payload_idx,
                reference_idx,
                ..
            } => {
                let after_idx = UtteranceIdx(*payload_idx);
                let before_idx = UtteranceIdx(*reference_idx);
                deltas.push(classify_same_words(before, before_idx, after, after_idx));
                i += 1;
            }
            AlignResult::ExtraPayload { payload_idx, .. } => {
                // Check if the next item is ExtraReference — that's a substitution pair
                if let Some(AlignResult::ExtraReference { reference_idx, .. }) =
                    alignment.get(i + 1)
                {
                    let after_idx = UtteranceIdx(*payload_idx);
                    let before_idx = UtteranceIdx(*reference_idx);
                    let tc = timing_differs(before, before_idx, after, after_idx);
                    deltas.push(UtteranceDelta::WordsChanged {
                        before_idx,
                        after_idx,
                        timing_changed: tc,
                    });
                    i += 2; // consume both
                } else {
                    // Pure insertion
                    deltas.push(UtteranceDelta::Inserted {
                        after_idx: UtteranceIdx(*payload_idx),
                    });
                    i += 1;
                }
            }
            AlignResult::ExtraReference { reference_idx, .. } => {
                // Check if the next item is ExtraPayload — reversed substitution pair
                if let Some(AlignResult::ExtraPayload { payload_idx, .. }) = alignment.get(i + 1) {
                    let after_idx = UtteranceIdx(*payload_idx);
                    let before_idx = UtteranceIdx(*reference_idx);
                    let tc = timing_differs(before, before_idx, after, after_idx);
                    deltas.push(UtteranceDelta::WordsChanged {
                        before_idx,
                        after_idx,
                        timing_changed: tc,
                    });
                    i += 2; // consume both
                } else {
                    // Pure deletion
                    deltas.push(UtteranceDelta::Deleted {
                        before_idx: UtteranceIdx(*reference_idx),
                    });
                    i += 1;
                }
            }
        }
    }

    // Sort deltas by after_idx (for insertions/matches) then before_idx (for deletions),
    // so they appear in document order.
    deltas.sort_by(|a, b| {
        let a_order = a.after_idx().map(|i| i.raw()).unwrap_or(usize::MAX);
        let b_order = b.after_idx().map(|i| i.raw()).unwrap_or(usize::MAX);
        a_order.cmp(&b_order).then_with(|| {
            let a_before = a.before_idx().map(|i| i.raw()).unwrap_or(usize::MAX);
            let b_before = b.before_idx().map(|i| i.raw()).unwrap_or(usize::MAX);
            a_before.cmp(&b_before)
        })
    });

    deltas
}
