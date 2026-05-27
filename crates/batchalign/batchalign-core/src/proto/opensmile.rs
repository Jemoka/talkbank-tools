//! openSMILE proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::OpenSmileInput,
//! OpenSmileOutput`. !!!

use crate::register_proto_schema;
use crate::utils::PreparedAudio;
use crate::metrics::MetricsTable;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Input: audio bytes + feature-set selector.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct OpenSmileInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Decoded PCM bytes.
    pub audio: PreparedAudio,
    /// openSMILE feature set ID, e.g. `"eGeMAPSv02"`.
    pub feature_set: SmolStr,
}

/// Output: a long-format metrics table to ship into a `MetricsArtifact`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct OpenSmileOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Echoes input's feature-set selector for downstream provenance.
    pub feature_set: SmolStr,
    /// Long-format metrics table.
    pub table: MetricsTable,
}

register_proto_schema!(OpenSmileInput);
register_proto_schema!(OpenSmileOutput);
