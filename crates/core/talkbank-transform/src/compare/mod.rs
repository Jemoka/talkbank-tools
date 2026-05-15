//! Transcript comparison: main vs gold-standard reference.
//!
//! Extracts words from both transcripts, normalizes them via
//! [`wer_conform::conform_words`], runs DP alignment, and produces
//! per-utterance comparison annotations with accuracy metrics.
//!
//! This module keeps compare concerns split by responsibility:
//! - [`model`] defines the workflow data structures
//! - [`engine`] owns alignment and bundle construction
//! - [`metrics`] owns aggregate counters and CSV materialization
//! - [`serialize`] owns `%xsrep` / `%xsmor` rendering
//! - [`materialize`] owns CHAT-tier injection and gold projection
//!
//! This is the Rust implementation of Python's `CompareEngine` +
//! `CompareAnalysisEngine` from batchalign2.

mod engine;
mod materialize;
mod metrics;
mod model;
mod serialize;

pub use self::engine::compare;
pub use self::materialize::{clear_comparison, inject_comparison, project_gold_structurally};
pub use self::metrics::{
    CompareCsvHeader, CompareMetricName, CompareMetricValue, CompareMetricsCsvRow,
    CompareMetricsCsvTable, ComparePosMetricKind, format_metrics_csv,
};
pub use self::model::{
    CompareMetrics, CompareResult, CompareStatus, CompareToken, ComparisonBundle, GoldWordMatch,
    PosErrorCounts, UtteranceComparison,
};
pub use self::serialize::{
    ComparePosLabel, CompareSerializationError, CompareSurfaceToken, CompareTierItem,
    CompareTierLabel, CompareTierMarker, CompareUserDefinedTier, XsmorTierContent,
    XsrepTierContent, format_xsmor, format_xsrep,
};

#[cfg(test)]
mod tests;
