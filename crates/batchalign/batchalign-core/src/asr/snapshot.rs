//! Per-stage snapshot of the ASR post-processing pipeline.
//!
//! Captures intermediate word/utterance lists at each pipeline stage
//! so callers (notably `batchalign`'s trace store) can render
//! per-stage diagnostics without having to re-run the pipeline.
//!
//! Threading is opt-in: every snapshot-aware function takes
//! `Option<&mut AsrPipelineSnapshot>`. Production callers that don't
//! need traces pass `None` and pay no allocation cost (the snapshot
//! fields are populated only on `Some`).
//!
//! Stages captured here mirror the stages described in the
//! `AsrPipelineTrace` shape exposed by `batchalign::types::traces`.
//! Conversion from `AsrPipelineSnapshot` to `AsrPipelineTrace` is done
//! by the caller; this crate stays free of trace-format coupling so the
//! lib remains usable independently of the server's trace store.

use serde::{Deserialize, Serialize};

use super::{AsrElement, AsrWord, Utterance};

/// One per-file snapshot of the ASR post-processing pipeline.
///
/// All fields default to empty. Snapshot-aware pipeline functions
/// populate them in-place via `&mut Option<AsrPipelineSnapshot>`-style
/// threading. Use [`AsrPipelineSnapshot::default`] to start a capture,
/// then pass `Some(&mut snapshot)` into the snapshot-aware variants of
/// `prepare_words_pre_expansion` and `finalize_words_to_chunks`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AsrPipelineSnapshot {
    /// Stage 0: raw ASR elements as received from the provider.
    /// Captured by the caller before invoking the pipeline.
    pub raw_elements: Vec<AsrElement>,
    /// Stage 1: after `merge_compounds`.
    pub after_compound_merge: Vec<AsrElement>,
    /// Stage 2: after `extract_timed_words` (raw seconds → ms timing).
    pub after_timing_extract: Vec<AsrWord>,
    /// Stage 3: after `split_multiword_tokens` and `split_percent_suffix_words`.
    /// Includes the post-Stage-3 boundary-quote re-strip (Stage 3c).
    pub after_multiword_split: Vec<AsrWord>,
    /// Stage 4: after per-word number expansion.
    pub after_number_expand: Vec<AsrWord>,
    /// Stage 4b: after Cantonese normalization. `None` for non-yue.
    pub after_cantonese_norm: Option<Vec<AsrWord>>,
    /// Stage 5: after long-turn splitting (one inner Vec per turn chunk).
    pub after_long_turn_split: Vec<Vec<AsrWord>>,
    /// Stage 6: final retokenized utterances.
    pub final_utterances: Vec<Utterance>,
}
