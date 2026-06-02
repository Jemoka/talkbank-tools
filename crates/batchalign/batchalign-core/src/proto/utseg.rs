//! Utterance-segmentation proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::UtSegInput,
//! UtSegOutput, UtteranceSpan`. !!!

use crate::cache::{hash_serialized, CacheKey};
use crate::proto::asr::{AsrSegment, AsrWord, LanguageSpec};
use crate::register_proto_schema;
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

impl CacheKey for UtSegInput {
    /// Excludes `source_id`. Same raw segments + language + fallback flag
    /// hashes to the same key.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            segments: &'a [AsrSegment],
            language: &'a LanguageSpec,
            stanza_fallback: bool,
        }
        hash_serialized(
            &K {
                segments: &self.segments,
                language: &self.language,
                stanza_fallback: self.stanza_fallback,
            },
            hasher,
        );
    }
}

/// Output: utterance-bounded spans.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UtSegOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Segmented utterances.
    pub utterances: Vec<UtteranceSpan>,
}

register_proto_schema!(UtteranceSpan);
register_proto_schema!(UtSegInput);
register_proto_schema!(UtSegOutput);
