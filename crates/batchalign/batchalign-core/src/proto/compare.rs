//! Compare proto types.
//!
//! !!! HAND-MIRRORED with `python/batchalign/_core/proto.py::CompareInput,
//! CompareOutput`. !!!
//!
//! The Compare task aligns a candidate transcript (`main`) against a gold
//! reference (`gold`) and emits the annotated main transcript plus a JSON
//! summary table (WER, insertions, deletions, etc.). The proto crosses the
//! backend boundary as CHAT text — the runner re-parses the annotated text
//! back into a validated AST.

use crate::cache::{CacheKey, hash_serialized};
use crate::register_proto_schema;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input: serialized main + gold transcripts.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CompareInput {
    /// Identity dedupe key (typically the main file's stem).
    pub source_id: SourceId,
    /// CHAT-serialized main transcript.
    pub main_chat: String,
    /// CHAT-serialized gold reference.
    pub gold_chat: String,
}

impl CacheKey for CompareInput {
    /// Excludes `source_id`. The result is fully determined by the two
    /// CHAT payloads, so identical main+gold pairs hit the same entry
    /// even when supplied under different file names.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            main_chat: &'a str,
            gold_chat: &'a str,
        }
        hash_serialized(
            &K {
                main_chat: &self.main_chat,
                gold_chat: &self.gold_chat,
            },
            hasher,
        );
    }
}

/// Output: annotated main + a wide-format metrics row mirroring BA2's
/// `compare.csv` shape. The runner turns these into a `BAValue::Cons` list
/// so a single Compare invocation emits both a `<source>.cha` and a
/// `<source>.compare.csv`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CompareOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// CHAT-serialized annotated main (with `%xsrep:` / `%xsmor:` / `%xcmp:`
    /// tiers and a `@Comment: ba.compare.summary:` header).
    pub annotated_main: String,
    /// JSON summary table — kept for backwards compatibility with the
    /// `@Comment: ba.compare.summary:` header. Same numbers as the columns
    /// in `metrics`, just JSON instead of long-format.
    pub metrics_json: String,
    /// One CSV-shaped metrics row carrying BA2's columns plus `cwer`: `file`,
    /// `wer`, `cwer`, `accuracy`, `matches`, `insertions`, `deletions`,
    /// `total_gold_words`, `total_main_words`, plus per-POS quartets (`NOUN:matches`,
    /// `NOUN:insertions`, `NOUN:deletions`, `NOUN:total`, …). The driver
    /// wraps this in a `MetricsArtifact { kind: Compare }` so the CSV
    /// lands at `<source>.compare.csv` next to the annotated `<source>.cha`.
    pub metrics: CompareMetrics,
}

/// Structured per-file metrics. Ports BA2's `CompareAnalysisEngine` output
/// (`batchalign/pipelines/analysis/compare.py::analyze`) — one row per file
/// with WER + per-POS breakdown.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct CompareMetrics {
    pub file_label: String,
    pub wer: f64,
    pub cwer: f64,
    pub accuracy: f64,
    pub matches: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub total_gold_words: u32,
    pub total_main_words: u32,
    /// Per-POS quartet entries, ordered alphabetically by POS code so the
    /// CSV output is reproducible.
    pub per_pos: Vec<CompareMetricsPos>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CompareMetricsPos {
    pub pos: String,
    pub matches: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub total: u32,
}

register_proto_schema!(CompareInput);
register_proto_schema!(CompareMetricsPos);
register_proto_schema!(CompareMetrics);
register_proto_schema!(CompareOutput);
