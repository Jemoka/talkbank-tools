//! Metrics artifacts produced by terminal tasks (openSMILE, AVQI, Compare).
//!
//! The engine crate's `metrics_writer` knows how to render this to CSV/Parquet
//! via polars. Core stays polars-free: the long-format table is a plain
//! serde-friendly `Vec<MetricsRow>` here, the engine builds a `polars::DataFrame`
//! at the boundary.
//!
//! Per `spec2.md` §20.

use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use smol_str::SmolStr;
use std::collections::BTreeMap;

/// What kind of metrics this artifact represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MetricsKind {
    /// openSMILE eGeMAPS / ComParE features.
    Opensmile,
    /// AVQI acoustic voice quality index.
    Avqi,
    /// Compare task — long-format table with per-utterance and summary rows.
    Compare,
    /// Benchmark probe row.
    Benchmark,
    /// Free-form; the producer's `SmolStr` name is the discriminator.
    Custom,
}

/// One row in a long-format metrics table. The `row_kind` column lets a
/// single artifact carry both per-utterance and summary rows (see Compare).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MetricsRow {
    /// Identifies the row's semantic class within the artifact
    /// (e.g. `"per_utterance"`, `"summary"`).
    pub row_kind: SmolStr,
    /// Column → value. `BTreeMap` for deterministic ordering in tests.
    pub columns: BTreeMap<String, JsonValue>,
}

/// A long-format metrics table — wire-friendly, polars-free.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct MetricsTable {
    /// Stable column order (engine writers honor this for CSV).
    pub schema: Vec<String>,
    /// One row per data point.
    pub rows: Vec<MetricsRow>,
}

/// What a terminal task produces. The engine's `metrics_writer` converts
/// `table` into a polars `DataFrame` at the boundary.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MetricsArtifact {
    /// Source identity (same as the input CHAT / media).
    pub source_id: SourceId,
    /// Producer string: e.g. `"opensmile:eGeMAPSv02"`, `"avqi:v1"`, …
    pub producer: SmolStr,
    /// Discriminator for write-routing.
    pub kind: MetricsKind,
    /// The data itself in long format.
    pub table: MetricsTable,
}
