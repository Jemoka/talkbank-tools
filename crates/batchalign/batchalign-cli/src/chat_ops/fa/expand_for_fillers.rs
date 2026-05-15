//! Pre-FA utterance bullet expansion for edge fillers.
//!
//! When a filler (`&-you_know`, `&-um`) sits at the start or end of an
//! utterance, the UTR-assigned bullet may be too narrow — ASR engines
//! typically don't transcribe fillers, so UTR never "sees" them. The
//! filler's audio lives in the gap between utterances, outside the bullet.
//!
//! This pass expands utterance bullets into adjacent gaps so the FA engine
//! receives audio covering the full utterance including edge fillers.
//! Runs after UTR but before FA grouping.

use talkbank_model::alignment::helpers::{TierDomain, WordItem, walk_words};
use talkbank_model::model::{ChatFile, Line, UtteranceContent, WordCategory};

/// Maximum expansion (ms) into a gap for a single edge filler.
///
/// Fillers in natural speech typically last 200–600 ms. We cap at 1500 ms
/// to avoid consuming too much inter-utterance silence even when large gaps
/// exist (e.g., long speaker turn transitions). Separate from the
/// group-level `TRAILING_GAP_EXTENSION_MS` in `grouping.rs` — this
/// adjusts individual utterance bullets before grouping.
const MAX_FILLER_EXPANSION_MS: u64 = 1500;

/// One utterance's summary used by the mutation pass:
/// `(bullet_ms_bounds, has_leading_filler, has_trailing_filler)`.
type UtteranceFillerSummary = (Option<(u64, u64)>, bool, bool);

/// Expand utterance bullets to cover edge fillers in inter-utterance gaps.
///
/// ASR engines don't transcribe fillers, so UTR-assigned bullets stop at
/// the last recognized word. Fillers at utterance edges are audible in the
/// gap but outside the bullet. This expansion reclaims that audio for FA.
pub fn expand_bullets_for_edge_fillers(chat_file: &mut ChatFile) {
    // Single pass: collect boundaries + edge filler status together.
    let utterance_data: Vec<UtteranceFillerSummary> = chat_file
        .lines
        .iter()
        .filter_map(|line| match line {
            Line::Utterance(u) => {
                let boundary = u
                    .main
                    .content
                    .bullet
                    .as_ref()
                    .map(|b| (b.timing.start_ms, b.timing.end_ms));
                let (leading, trailing) = detect_edge_fillers(&u.main.content.content);
                Some((boundary, leading, trailing))
            }
            _ => None,
        })
        .collect();

    // Mutation pass: expand bullets using collected neighbor data.
    let mut utt_idx = 0;
    for line in &mut chat_file.lines {
        let utt = match line {
            Line::Utterance(u) => u,
            _ => continue,
        };

        let bullet = match &mut utt.main.content.bullet {
            Some(b) => b,
            None => {
                utt_idx += 1;
                continue;
            }
        };

        let (_, has_leading, has_trailing) = utterance_data[utt_idx];

        // Expand start backward: reclaim gap before this utterance for a leading filler.
        if has_leading {
            let prev_end = if utt_idx > 0 {
                utterance_data[utt_idx - 1].0.map(|(_, end)| end)
            } else {
                Some(0)
            };
            if let Some(prev_end) = prev_end
                && bullet.timing.start_ms > prev_end
            {
                let gap = bullet.timing.start_ms - prev_end;
                // expansion ≤ gap/2, so start_ms -= expansion cannot underflow.
                let expansion = (gap / 2).min(MAX_FILLER_EXPANSION_MS);
                bullet.timing.start_ms -= expansion;
            }
        }

        // Expand end forward: reclaim gap after this utterance for a trailing filler.
        if has_trailing {
            let next_start = if utt_idx + 1 < utterance_data.len() {
                utterance_data[utt_idx + 1].0.map(|(start, _)| start)
            } else {
                None
            };
            if let Some(next_start) = next_start
                && next_start > bullet.timing.end_ms
            {
                let gap = next_start - bullet.timing.end_ms;
                let expansion = (gap / 2).min(MAX_FILLER_EXPANSION_MS);
                bullet.timing.end_ms += expansion;
            }
        }

        utt_idx += 1;
    }
}

/// Detect whether the first and last alignable words are fillers.
///
/// Uses domain-gated `walk_words` with `TierDomain::Wor` so only
/// alignable words reach the closure — no redundant `counts_for_tier` check.
pub(crate) fn detect_edge_fillers(content: &[UtteranceContent]) -> (bool, bool) {
    let mut first: Option<bool> = None;
    let mut last: Option<bool> = None;

    walk_words(content, Some(TierDomain::Wor), &mut |leaf| {
        let is_filler = match leaf {
            WordItem::Word(w) => w.category == Some(WordCategory::Filler),
            WordItem::ReplacedWord(r) => r.word.category == Some(WordCategory::Filler),
            WordItem::Separator(_) => return,
        };
        if first.is_none() {
            first = Some(is_filler);
        }
        last = Some(is_filler);
    });

    (first.unwrap_or(false), last.unwrap_or(false))
}
