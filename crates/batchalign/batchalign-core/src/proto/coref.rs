//! Coreference proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::CorefInput,
//! CorefOutput`. !!!

use crate::cache::{CacheKey, hash_serialized};
use crate::register_proto_schema;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input: per-utterance text and (optional) speaker tags, in CHAT order.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CorefInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Utterance text in CHAT order.
    pub utterances: Vec<String>,
    /// Optional per-utterance speaker codes (e.g. `"CHI"`, `"MOT"`).
    #[serde(default)]
    pub speakers: Vec<String>,
}

impl CacheKey for CorefInput {
    /// Excludes `source_id`. Same utterance list + speaker tags → same key.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            utterances: &'a [String],
            speakers: &'a [String],
        }
        hash_serialized(
            &K {
                utterances: &self.utterances,
                speakers: &self.speakers,
            },
            hasher,
        );
    }
}

/// Output: per-utterance `%coref` content strings (no `%coref:` label), index-aligned.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CorefOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Coreference annotation lines, same length as input utterances.
    pub annotations: Vec<String>,
}

register_proto_schema!(CorefInput);
register_proto_schema!(CorefOutput);
