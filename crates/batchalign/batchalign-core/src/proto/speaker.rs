//! Speaker-diarization proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::SpeakerInput,
//! SpeakerOutput, Diarization, DiarizationSegment`. !!!

use crate::cache::{CacheKey, hash_serialized};
use crate::register_proto_schema;
use crate::utils::SourceId;
use crate::utils::{PreparedAudio, SpeakerLabel};
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

impl CacheKey for SpeakerInput {
    /// Excludes `source_id`. Same audio + same speaker-count hint hashes
    /// to the same key across files.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            audio: &'a PreparedAudio,
            num_speakers: u32,
        }
        hash_serialized(
            &K {
                audio: &self.audio,
                num_speakers: self.num_speakers,
            },
            hasher,
        );
    }
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

register_proto_schema!(SpeakerInput);
register_proto_schema!(DiarizationSegment);
register_proto_schema!(Diarization);
register_proto_schema!(SpeakerOutput);
