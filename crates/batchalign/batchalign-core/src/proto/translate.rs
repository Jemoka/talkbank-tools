//! Translation proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::TranslateInput,
//! TranslateOutput`. !!!

use crate::proto::asr::LanguageSpec;
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

/// Output: translations, index-aligned with input utterances.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TranslateOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Translated utterances, same length as input.
    pub utterances: Vec<String>,
}
