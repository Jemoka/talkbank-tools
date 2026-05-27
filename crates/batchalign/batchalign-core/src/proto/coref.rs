//! Coreference proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::CorefInput,
//! CorefOutput`. !!!

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
