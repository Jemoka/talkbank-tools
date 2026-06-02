//! Translation proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::TranslateInput,
//! TranslateOutput`. !!!

use crate::cache::{hash_serialized, CacheKey};
use crate::proto::asr::LanguageSpec;
use crate::register_proto_schema;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Input: per-utterance source text and source/target language hints.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TranslateInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Utterance source text in CHAT order.
    pub utterances: Vec<String>,
    /// Source language hint — frequently `LanguageSpec::PerFile`.
    pub source: LanguageSpec,
    /// Target language ISO-639-3 code (e.g. `"eng"`).
    pub target: SmolStr,
}

impl CacheKey for TranslateInput {
    /// Excludes `source_id`. Same utterance list + source/target language
    /// pair → same key.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            utterances: &'a [String],
            source: &'a LanguageSpec,
            target: &'a SmolStr,
        }
        hash_serialized(
            &K {
                utterances: &self.utterances,
                source: &self.source,
                target: &self.target,
            },
            hasher,
        );
    }
}

/// Output: translations, index-aligned with input utterances.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TranslateOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Translated utterances, same length as input.
    pub utterances: Vec<String>,
}

register_proto_schema!(TranslateInput);
register_proto_schema!(TranslateOutput);
