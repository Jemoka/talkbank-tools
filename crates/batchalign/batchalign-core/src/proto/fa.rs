//! Forced-alignment proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::FaInput, FaOutput`. !!!

use crate::cache::{hash_serialized, CacheKey};
use crate::proto::asr::{AsrSegment, LanguageSpec};
use crate::register_proto_schema;
use crate::utils::PreparedAudio;
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

impl CacheKey for FaInput {
    /// Excludes `source_id`. Same audio + same utterance text/bounds +
    /// same language must hash to the same key.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            audio: &'a PreparedAudio,
            utterances: &'a [AsrSegment],
            language: &'a LanguageSpec,
        }
        hash_serialized(
            &K {
                audio: &self.audio,
                utterances: &self.utterances,
                language: &self.language,
            },
            hasher,
        );
    }
}

/// Output: same utterances, with refined `words[*].start_ms / end_ms`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FaOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Utterances with refined word timings.
    pub utterances: Vec<AsrSegment>,
}

register_proto_schema!(FaInput);
register_proto_schema!(FaOutput);
