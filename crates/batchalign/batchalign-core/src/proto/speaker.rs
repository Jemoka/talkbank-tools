//! Speaker-diarization proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::SpeakerInput,
//! SpeakerOutput, Diarization, DiarizationSegment`. !!!

use crate::utils::{PreparedAudio, SpeakerLabel};
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input: audio bytes plus an optional speaker-count hint.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Decoded PCM bytes.
    pub audio: PreparedAudio,
    /// Hint; 0 = auto.
    #[serde(default)]
    pub num_speakers: u32,
}

/// One diarized segment.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DiarizationSegment {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Backend-emitted speaker label.
    pub speaker: SpeakerLabel,
}

/// Full diarization result.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Diarization {
    /// Time-ordered speaker assignments.
    pub segments: Vec<DiarizationSegment>,
}

/// Output: just the diarization.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Segmentation result.
    pub diarization: Diarization,
}
