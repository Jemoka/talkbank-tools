//! Response parsing and deterministic alignment for FA results.

use crate::chat_ops::nlp::{FaIndexedTiming, FaRawResponse, FaRawToken};

use super::{FaTimingMode, FaWord, WordTiming};

/// Typed error returned by [`parse_fa_response`].
///
/// Wave 5 of the morphotag reconciliation architecture replaced the
/// previous `Result<_, String>` return with this enum so failure modes
/// can be discriminated at the call site without string parsing. The
/// two variants correspond to structurally distinct problems:
///
/// - `JsonParse` — worker returned text that isn't a valid FA response
///   payload. This is a worker-protocol bug.
/// - `IndexedCountMismatch` — worker returned the wrong number of
///   per-word timings (the FA equivalent of morphotag's
///   `MisalignmentBug`). Always a worker-contract bug — the Python FA
///   worker is supposed to emit one `Option<FaIndexedTiming>` per input
///   `FaWord` for the indexed-word-level response shape.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FaAlignmentError {
    /// The worker's JSON response could not be deserialized into
    /// [`FaRawResponse`](crate::chat_ops::nlp::FaRawResponse).
    #[error("failed to parse raw FA response: {message}")]
    JsonParse {
        /// Underlying serde error rendered as a string (preserved
        /// through the `Clone` boundary; `serde_json::Error` itself is
        /// not `Clone`).
        message: String,
    },
    /// The worker returned an indexed-word-level response whose length
    /// disagrees with the number of input words.
    #[error(
        "FA indexed-response length mismatch: expected {expected} timings for \
         {expected} words, got {actual}"
    )]
    IndexedCountMismatch {
        /// Number of words sent to the worker (expected count).
        expected: usize,
        /// Number of timings actually returned.
        actual: usize,
    },
}

/// Parse the JSON response from the FA callback and align it with original words.
///
/// # Errors
///
/// Returns [`FaAlignmentError::JsonParse`] if the response isn't valid
/// FA JSON, or [`FaAlignmentError::IndexedCountMismatch`] if the
/// indexed-word-level variant returned the wrong count.
pub fn parse_fa_response(
    json_str: &str,
    original_words: &[FaWord],
    audio_start_ms: u64,
    timing_mode: FaTimingMode,
) -> Result<Vec<Option<WordTiming>>, FaAlignmentError> {
    let raw_resp: FaRawResponse =
        serde_json::from_str(json_str).map_err(|e| FaAlignmentError::JsonParse {
            message: e.to_string(),
        })?;

    match raw_resp {
        FaRawResponse::IndexedWordLevel { indexed_timings } => {
            if indexed_timings.len() != original_words.len() {
                return Err(FaAlignmentError::IndexedCountMismatch {
                    expected: original_words.len(),
                    actual: indexed_timings.len(),
                });
            }
            Ok(apply_indexed_timings(
                original_words,
                &indexed_timings,
                audio_start_ms,
            ))
        }
        FaRawResponse::TokenLevel { tokens } => Ok(align_token_timings(
            original_words,
            &tokens,
            audio_start_ms,
            timing_mode,
        )),
    }
}

/// Apply index-aligned word timings (no DP remapping required).
fn apply_indexed_timings(
    original: &[FaWord],
    indexed_timings: &[Option<FaIndexedTiming>],
    audio_offset_ms: u64,
) -> Vec<Option<WordTiming>> {
    let mut results = vec![None; original.len()];
    for (idx, maybe_timing) in indexed_timings.iter().enumerate() {
        if let Some(timing) = maybe_timing {
            results[idx] = Some(WordTiming {
                start_ms: timing.start_ms + audio_offset_ms,
                end_ms: timing.end_ms + audio_offset_ms,
            });
        }
    }

    results
}

fn normalize_fa_alignment_unit(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

/// Align token-level onset times (typical for Whisper) with original CHAT words.
///
/// This path is deterministic only: it stitches normalized Whisper tokens onto
/// normalized transcript words in order. If stitching fails, unmatched words are
/// left as `None` (no DP remapping).
fn align_token_timings(
    original: &[FaWord],
    tokens: &[FaRawToken],
    audio_offset_ms: u64,
    timing_mode: FaTimingMode,
) -> Vec<Option<WordTiming>> {
    if original.is_empty() || tokens.is_empty() {
        return vec![None; original.len()];
    }

    let mut word_norms = Vec::with_capacity(original.len());
    for word in original {
        let norm = normalize_fa_alignment_unit(word.text.as_str());
        if norm.is_empty() {
            return vec![None; original.len()];
        }
        word_norms.push(norm);
    }

    let mut token_norms = Vec::new();
    let mut token_starts_ms = Vec::new();
    for token in tokens {
        let token_text = token.text.trim();
        if token_text.starts_with("<|") && token_text.ends_with("|>") {
            continue;
        }
        let norm = normalize_fa_alignment_unit(token_text);
        if norm.is_empty() {
            continue;
        }
        token_norms.push(norm);
        token_starts_ms.push((token.time_s * 1000.0) as u64 + audio_offset_ms);
    }

    if token_norms.is_empty() {
        return vec![None; original.len()];
    }

    let mut results = vec![None; original.len()];
    let mut token_idx = 0usize;
    let mut matched_words = 0usize;

    for (word_idx, word_norm) in word_norms.iter().enumerate() {
        if token_idx >= token_norms.len() {
            break;
        }

        let start_ms = token_starts_ms[token_idx];
        let mut acc = String::new();
        let mut matched = false;

        while token_idx < token_norms.len() {
            let mut next_acc = acc.clone();
            next_acc.push_str(token_norms[token_idx].as_str());
            if !word_norm.starts_with(next_acc.as_str()) {
                break;
            }
            acc = next_acc;
            token_idx += 1;
            if acc == *word_norm {
                let end_ms = match timing_mode {
                    FaTimingMode::WithPauses => token_starts_ms
                        .get(token_idx)
                        .copied()
                        .unwrap_or(start_ms + 500)
                        .max(start_ms + 1),
                    FaTimingMode::Continuous => start_ms,
                };
                results[word_idx] = Some(WordTiming { start_ms, end_ms });
                matched_words += 1;
                matched = true;
                break;
            }
        }

        if !matched {
            break;
        }
    }

    if matched_words < original.len() {
        tracing::warn!(
            matched_words,
            total_words = original.len(),
            token_count = token_norms.len(),
            "deterministic token stitching did not cover all words; leaving unmatched words untimed"
        );
    }

    results
}
