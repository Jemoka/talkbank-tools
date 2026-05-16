//! AVQI proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::AvqiInput,
//! AvqiOutput`. !!!

use crate::utils::PreparedAudio;
use crate::metrics::MetricsTable;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input: audio bytes only.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvqiInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Decoded PCM bytes.
    pub audio: PreparedAudio,
}

/// Output: AVQI score(s) as a long-format metrics table.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvqiOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Long-format metrics table (typically one row with the score columns).
    pub table: MetricsTable,
}
