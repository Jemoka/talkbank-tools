//! Trace data structures for algorithm visualization.
//!
//! Orchestrators return structured result types (e.g. [`super::results::FaResult`])
//! that always carry intermediate data.  When `debug_traces` is enabled for a
//! job, the dispatch layer converts these results into trace structs and stores
//! them in the ephemeral [`crate::trace_store::TraceStore`].  The dashboard
//! fetches them via `GET /jobs/{id}/traces`.
//!
//! When `debug_traces` is off (the default), structured results are still
//! returned but traces are not stored — no extra memory is used.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::api::{DurationMs, DurationSeconds};

// ---------------------------------------------------------------------------
// Top-level containers
// ---------------------------------------------------------------------------

/// All algorithm traces collected for a completed job.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct JobTraces {
    /// Per-file traces, keyed by file index (0-based).
    pub files: BTreeMap<usize, FileTraces>,
}

/// Algorithm traces for a single file within a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct FileTraces {
    /// Original filename (e.g. "01DM_18.cha").
    pub filename: crate::api::DisplayPath,
    /// DP alignment traces (one per alignment call: FA, retokenize, WER).
    pub dp_alignments: Vec<DpAlignmentTrace>,
    /// ASR post-processing pipeline trace (transcribe jobs only).
    pub asr_pipeline: Option<AsrPipelineTrace>,
    /// Forced alignment timeline trace (align jobs only).
    pub fa_timeline: Option<FaTimelineTrace>,
    /// Retokenization traces (one per utterance that was retokenized).
    pub retokenizations: Vec<RetokenizationTrace>,
}

// ---------------------------------------------------------------------------
// DP Alignment
// ---------------------------------------------------------------------------

/// Full matrix + traceback for a single `align_small` invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct DpAlignmentTrace {
    /// What triggered this alignment (e.g. "fa_whisper", "retokenize", "wer").
    pub context: String,
    /// Payload sequence (left side).
    pub payload: Vec<String>,
    /// Reference sequence (top side).
    pub reference: Vec<String>,
    /// Match mode used ("exact" or "case_insensitive").
    pub match_mode: String,
    /// Number of prefix elements stripped before DP.
    pub prefix_stripped: usize,
    /// Number of suffix elements stripped before DP.
    pub suffix_stripped: usize,
    /// Flat cost matrix (row-major, `(ref_len+1) * (pay_len+1)` entries).
    pub cost_matrix: Vec<usize>,
    /// Traceback path through the cost matrix.
    pub traceback: Vec<AlignStepTrace>,
    /// Final alignment result.
    pub result: Vec<AlignResultTrace>,
}

/// A single step in the DP traceback path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct AlignStepTrace {
    /// Action taken: "match", "substitution", "extra_payload", "extra_reference".
    pub action: String,
    /// Row index in the cost matrix.
    pub i: usize,
    /// Column index in the cost matrix.
    pub j: usize,
}

/// A single item in the alignment result (matches `AlignResult` enum).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct AlignResultTrace {
    /// "match", "extra_payload", or "extra_reference".
    pub kind: String,
    /// The string key.
    pub key: String,
    /// Index into payload (present for match and extra_payload).
    pub payload_idx: Option<usize>,
    /// Index into reference (present for match and extra_reference).
    pub reference_idx: Option<usize>,
}

// ---------------------------------------------------------------------------
// ASR Pipeline
// ---------------------------------------------------------------------------

/// Intermediate word lists at each stage of ASR post-processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct AsrPipelineTrace {
    /// Stage 0: raw tokens from the ASR worker.
    pub raw_tokens: Vec<AsrTokenTrace>,
    /// Stage 1: after compound merging.
    pub after_compound_merge: Vec<WordTrace>,
    /// Stage 2: after timed word extraction (seconds → ms).
    pub after_timing_extract: Vec<TimedWordTrace>,
    /// Stage 3: after multi-word splitting.
    pub after_multiword_split: Vec<TimedWordTrace>,
    /// Stage 4: after number expansion.
    pub after_number_expand: Vec<TimedWordTrace>,
    /// Stage 4b: after Cantonese normalization (only if lang=yue).
    pub after_cantonese_norm: Option<Vec<TimedWordTrace>>,
    /// Stage 5: after long-turn splitting (nested by turn).
    pub after_long_turn_split: Vec<Vec<TimedWordTrace>>,
    /// Stage 6: final utterances.
    pub final_utterances: Vec<UtteranceTrace>,
}

/// A raw ASR token (stage 0).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct AsrTokenTrace {
    /// Token text.
    pub value: String,
    /// Start time in seconds.
    pub ts: DurationSeconds,
    /// End time in seconds.
    pub end_ts: DurationSeconds,
    /// Token type ("text", "punctuation", etc.).
    pub token_type: String,
}

/// A word without timing (e.g. after compound merge).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct WordTrace {
    /// Word text.
    pub text: String,
}

/// A word with optional timing in milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct TimedWordTrace {
    /// Word text.
    pub text: String,
    /// Start time in ms (if known).
    pub start_ms: Option<i64>,
    /// End time in ms (if known).
    pub end_ms: Option<i64>,
}

/// A final utterance (stage 6).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct UtteranceTrace {
    /// Speaker index (0-based).
    pub speaker: usize,
    /// Words in the utterance.
    pub words: Vec<TimedWordTrace>,
}

// ---------------------------------------------------------------------------
// FA Timeline
// ---------------------------------------------------------------------------

/// Forced alignment trace: grouping, timing injection, and post-processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct FaTimelineTrace {
    /// Utterance groups for batched FA.
    pub groups: Vec<FaGroupTrace>,
    /// Pre-injection timings per group, per word (None = untimed).
    pub pre_injection_timings: Vec<Vec<Option<TimingTrace>>>,
    /// Post-injection timings after post-processing fixes.
    pub post_injection_timings: Vec<Vec<Option<TimingTrace>>>,
    /// Timing mode used ("continuous" or "with_pauses").
    pub timing_mode: String,
    /// Validation violations detected (e.g. E362, E704).
    pub violations: Vec<ViolationTrace>,
    /// Engine fallback events that occurred while aligning this file.
    pub fallback_events: Vec<FaFallbackEventTrace>,
}

/// A single FA group (time-windowed batch of utterances).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct FaGroupTrace {
    /// Audio start time in ms.
    pub audio_start_ms: DurationMs,
    /// Audio end time in ms.
    pub audio_end_ms: DurationMs,
    /// Utterance indices covered by this group.
    pub utterance_indices: Vec<usize>,
    /// Words in this group.
    pub words: Vec<String>,
}

/// One forced-alignment engine fallback that occurred for a single group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct FaFallbackEventTrace {
    /// Group index within the file-local FA grouping.
    pub group_index: usize,
    /// Engine originally requested by the Rust control plane.
    pub from_engine: String,
    /// Engine actually used for the retry.
    pub to_engine: String,
    /// Human-readable reason why the fallback was triggered.
    pub reason: String,
    /// Audio start time of the affected group in ms.
    pub audio_start_ms: DurationMs,
    /// Audio end time of the affected group in ms.
    pub audio_end_ms: DurationMs,
}

/// Start/end timing for a single word.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct TimingTrace {
    /// Start time in ms.
    pub start_ms: i64,
    /// End time in ms.
    pub end_ms: i64,
}

/// A validation violation detected during FA.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct ViolationTrace {
    /// Error code (e.g. "E362", "E704").
    pub code: String,
    /// Human-readable description.
    pub message: String,
    /// Utterance index where the violation was found.
    pub utterance_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Retokenization
// ---------------------------------------------------------------------------

/// Retokenization trace for a single utterance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct RetokenizationTrace {
    /// Utterance index in the file (0-based).
    pub utterance_index: usize,
    /// Original CHAT words.
    pub original_words: Vec<String>,
    /// Stanza tokens after retokenization.
    pub stanza_tokens: Vec<String>,
    /// Normalized concatenation of original words.
    pub normalized_original: String,
    /// Normalized concatenation of Stanza tokens.
    pub normalized_tokens: String,
    /// Word→token index mapping: `mapping[word_idx]` = list of token indices.
    pub mapping: Vec<Vec<usize>>,
    /// Whether the fallback (length-proportional) mapping was used.
    pub used_fallback: bool,
}
