//! Forced-alignment proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::FaInput, FaOutput`. !!!

use crate::utils::PreparedAudio;
use crate::proto::asr::{AsrSegment, LanguageSpec};
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input to a forced-alignment backend: the audio plus the already-segmented
/// utterances (text-only; FA fills word-level timings).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FaInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Decoded PCM bytes.
    pub audio: PreparedAudio,
    /// Per-utterance text + (currently rough) bounds. FA refines word
    /// timings inside `words`.
    pub utterances: Vec<AsrSegment>,
    /// Language hint, frequently `LanguageSpec::PerFile`.
    pub language: LanguageSpec,
}

/// Output: same utterances, with refined `words[*].start_ms / end_ms`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FaOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Utterances with refined word timings.
    pub utterances: Vec<AsrSegment>,
}
