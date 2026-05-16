//! Utterance-segmentation proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::UtSegInput,
//! UtSegOutput, UtteranceSpan`. !!!

use crate::proto::asr::{AsrSegment, AsrWord, LanguageSpec};
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One utterance-bounded span emitted by the UtSeg backend.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UtteranceSpan {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Joined utterance text.
    pub text: String,
    /// Words inside this utterance with their original ASR timings.
    pub words: Vec<AsrWord>,
}

/// Input: the raw ASR segment blob to segment.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UtSegInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Raw ASR segments (typically a single long monologue per speaker).
    pub segments: Vec<AsrSegment>,
    /// Language hint controlling the segmentation model.
    pub language: LanguageSpec,
    /// Whether to allow Stanza punctuation fallback if no model is available.
    #[serde(default)]
    pub stanza_fallback: bool,
}

/// Output: utterance-bounded spans.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UtSegOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Segmented utterances.
    pub utterances: Vec<UtteranceSpan>,
}
