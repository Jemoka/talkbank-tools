//! Structured result types for server-side orchestrators.
//!
//! Each orchestrator returns a rich result type that includes both the
//! serialized CHAT output and any intermediate data produced during
//! processing.  The dispatch layer decides what to write to disk vs.
//! what to store in the trace cache.

use crate::chat_ops::fa::FaTimingMode;
use crate::chat_ops::morphosyntax_ops::RetokenizationInfo;
use talkbank_transform::asr_postprocess::AsrPipelineSnapshot;

use super::traces::{
    AsrPipelineTrace, AsrTokenTrace, FaFallbackEventTrace, FaGroupTrace, FaTimelineTrace,
    RetokenizationTrace, TimedWordTrace, TimingTrace, UtteranceTrace, ViolationTrace, WordTrace,
};
use crate::api::DurationSeconds;

// ---------------------------------------------------------------------------
// Forced alignment
// ---------------------------------------------------------------------------

/// Structured result from [`crate::fa::process_fa`].
pub struct FaResult {
    /// Serialized CHAT text with timings injected.
    pub chat_text: String,
    /// FA groups that were processed.
    pub groups: Vec<FaGroupTrace>,
    /// Timings as returned by the worker, before post-processing.
    pub pre_injection_timings: Vec<Vec<Option<TimingTrace>>>,
    /// Timing mode used for this run.
    pub timing_mode: FaTimingMode,
    /// Post-validation violations.
    pub violations: Vec<ViolationTrace>,
    /// Engine fallback events captured during worker inference.
    pub fallback_events: Vec<FaFallbackEventTrace>,
}

impl FaResult {
    /// Convert into a [`FaTimelineTrace`] for dashboard visualization.
    pub fn into_timeline_trace(self) -> FaTimelineTrace {
        FaTimelineTrace {
            groups: self.groups,
            pre_injection_timings: self.pre_injection_timings,
            post_injection_timings: Vec::new(), // TODO Phase 4
            timing_mode: format!("{:?}", self.timing_mode),
            violations: self.violations,
            fallback_events: self.fallback_events,
        }
    }
}

// ---------------------------------------------------------------------------
// ASR pipeline trace conversion
// ---------------------------------------------------------------------------

/// Lossy conversion from the chat-ops-side per-stage snapshot to the
/// dashboard-facing `AsrPipelineTrace`.
///
/// Drops timing and structural detail not surfaced in the trace shape
/// (e.g. `AsrWord::kind`). The trace shape is the dashboard contract;
/// the snapshot is the wire-protocol-free internal capture.
pub fn snapshot_into_pipeline_trace(snapshot: AsrPipelineSnapshot) -> AsrPipelineTrace {
    AsrPipelineTrace {
        raw_tokens: snapshot
            .raw_elements
            .iter()
            .map(|e| AsrTokenTrace {
                value: e.value.as_str().to_owned(),
                ts: DurationSeconds(e.ts.as_f64()),
                end_ts: DurationSeconds(e.end_ts.as_f64()),
                token_type: format!("{:?}", e.kind).to_lowercase(),
            })
            .collect(),
        after_compound_merge: snapshot
            .after_compound_merge
            .iter()
            .map(|e| WordTrace {
                text: e.value.as_str().to_owned(),
            })
            .collect(),
        after_timing_extract: snapshot
            .after_timing_extract
            .iter()
            .map(asr_word_to_timed_trace)
            .collect(),
        after_multiword_split: snapshot
            .after_multiword_split
            .iter()
            .map(asr_word_to_timed_trace)
            .collect(),
        after_number_expand: snapshot
            .after_number_expand
            .iter()
            .map(asr_word_to_timed_trace)
            .collect(),
        after_cantonese_norm: snapshot
            .after_cantonese_norm
            .map(|words| words.iter().map(asr_word_to_timed_trace).collect()),
        after_long_turn_split: snapshot
            .after_long_turn_split
            .iter()
            .map(|chunk| chunk.iter().map(asr_word_to_timed_trace).collect())
            .collect(),
        final_utterances: snapshot
            .final_utterances
            .iter()
            .map(|u| UtteranceTrace {
                speaker: u.speaker.as_usize(),
                words: u.words.iter().map(asr_word_to_timed_trace).collect(),
            })
            .collect(),
    }
}

fn asr_word_to_timed_trace(w: &talkbank_transform::asr_postprocess::AsrWord) -> TimedWordTrace {
    TimedWordTrace {
        text: w.text.as_str().to_owned(),
        start_ms: w.start_ms,
        end_ms: w.end_ms,
    }
}

// ---------------------------------------------------------------------------
// Morphosyntax
// ---------------------------------------------------------------------------

/// Structured result from a single-file morphosyntax run.
pub struct MorphosyntaxResult {
    /// Serialized CHAT text with %mor/%gra injected.
    pub chat_text: String,
    /// Retokenization mappings (empty when retokenization is off).
    pub retokenizations: Vec<RetokenizationInfo>,
}

impl MorphosyntaxResult {
    /// Convert retokenization info into dashboard trace format.
    pub fn into_retokenization_traces(self) -> Vec<RetokenizationTrace> {
        self.retokenizations
            .into_iter()
            .map(|info| RetokenizationTrace {
                utterance_index: info.utterance_ordinal,
                original_words: info.original_words,
                stanza_tokens: info.stanza_tokens,
                normalized_original: String::new(), // not captured at this level
                normalized_tokens: String::new(),
                mapping: info.mapping,
                used_fallback: info.used_fallback,
            })
            .collect()
    }
}
