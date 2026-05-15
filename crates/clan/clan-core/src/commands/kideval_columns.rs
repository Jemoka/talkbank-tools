//! Column mapping between KidEval `.cut` database scores and [`SpeakerKideval`] fields.
//!
//! The `.cut` database stores ~25 fixed metrics followed by variable-length
//! morphosyntactic counts. This module defines the fixed column positions and
//! provides conversion functions between `SpeakerKideval` and flat score vectors.
//!
//! Column order is defined by the `0all_norms_with_columns.csv` reference file
//! in `lib/kideval/`.
//!
//! # Differences from CLAN
//!
//! This module has no direct CLAN equivalent — CLAN embeds column mapping
//! inline in `kideval.cpp`. Extracting it as a separate module enables reuse
//! by the VS Code extension and other API consumers.

use super::kideval::SpeakerKideval;
use crate::database::ComparisonResult;
use serde::Serialize;

/// Named column indices in the KidEval `.cut` database.
///
/// These match the `0all_norms_with_columns.csv` header (score columns only,
/// 0-indexed from the start of the numeric data line).
pub mod col {
    /// Total utterances (mWords in CLAN source).
    pub const TOTAL_UTTS: usize = 0;
    /// MLU utterances (morf in CLAN source).
    pub const MLU_UTTS: usize = 1;
    /// MLU in words.
    pub const MLU_WORDS: usize = 2;
    /// MLU in morphemes.
    pub const MLU_MORPHEMES: usize = 3;
    // 4-6: MLU50 variants (not computed by our command)
    /// Frequency types (unique words).
    pub const FREQ_TYPES: usize = 7;
    /// Frequency tokens (total words).
    pub const FREQ_TOKENS: usize = 8;
    /// Number of different words (NDW, 100-word sample).
    pub const NDW: usize = 9;
    // 10: NDW total
    /// VOCD-D optimum average.
    pub const VOCD: usize = 11;
    /// Verbs per utterance ratio.
    pub const VERBS_UTT: usize = 12;
    /// Word errors count.
    pub const WORD_ERRORS: usize = 13;
    // 14: Utterance errors
    // 15-16: retracing, repetition
    /// DSS utterance count.
    pub const DSS_UTTS: usize = 17;
    /// DSS score.
    pub const DSS: usize = 18;
    /// IPSyn utterance count.
    pub const IPSYN_UTTS: usize = 19;
    /// IPSyn total score.
    pub const IPSYN_TOTAL: usize = 20;
    /// Total morphemes on %mor tier.
    pub const MOR_WORDS: usize = 21;
}

/// A named comparison for a single KidEval measure.
#[derive(Debug, Clone, Serialize)]
pub struct KidevalMeasureComparison {
    /// Human-readable label (e.g., "MLU (words)").
    pub label: &'static str,
    /// The speaker's score.
    pub score: f64,
    /// Database population mean.
    pub db_mean: f64,
    /// Database population standard deviation.
    pub db_sd: f64,
    /// Z-score (standard deviations from norm). `None` if SD is zero.
    pub z_score: Option<f64>,
    /// Number of database entries used.
    pub db_n: usize,
}

/// Mapping entry: which `.cut` column corresponds to which speaker field.
struct ColumnMapping {
    label: &'static str,
    col_index: usize,
    extract: fn(&SpeakerKideval) -> f64,
}

/// All mappings between `SpeakerKideval` fields and `.cut` database columns.
const MAPPINGS: &[ColumnMapping] = &[
    ColumnMapping {
        label: "Utterances",
        col_index: col::TOTAL_UTTS,
        extract: |s| s.utterances as f64,
    },
    ColumnMapping {
        label: "Total words",
        col_index: col::FREQ_TOKENS,
        extract: |s| s.total_words as f64,
    },
    ColumnMapping {
        label: "NDW",
        col_index: col::NDW,
        extract: |s| s.ndw as f64,
    },
    ColumnMapping {
        label: "MLU (words)",
        col_index: col::MLU_WORDS,
        extract: |s| s.mlu_words,
    },
    ColumnMapping {
        label: "MLU (morphemes)",
        col_index: col::MLU_MORPHEMES,
        extract: |s| s.mlu_morphemes,
    },
    ColumnMapping {
        label: "VOCD",
        col_index: col::VOCD,
        extract: |s| s.vocd_score,
    },
    ColumnMapping {
        label: "DSS",
        col_index: col::DSS,
        extract: |s| s.dss_score,
    },
    ColumnMapping {
        label: "IPSyn",
        col_index: col::IPSYN_TOTAL,
        extract: |s| s.ipsyn_score as f64,
    },
    ColumnMapping {
        label: "Word errors",
        col_index: col::WORD_ERRORS,
        extract: |s| s.word_errors as f64,
    },
];

/// Extract the scores from a `SpeakerKideval` that have database column mappings,
/// and produce named comparisons from a [`ComparisonResult`].
///
/// This bridges the gap between the positional database comparison and the
/// typed KidEval output — selecting only the columns we compute and labeling them.
pub fn map_kideval_comparison(
    speaker: &SpeakerKideval,
    comparison: &ComparisonResult,
) -> Vec<KidevalMeasureComparison> {
    MAPPINGS
        .iter()
        .filter_map(|m| {
            let measure = comparison.measures.get(m.col_index)?;
            Some(KidevalMeasureComparison {
                label: m.label,
                score: (m.extract)(speaker),
                db_mean: measure.db_mean,
                db_sd: measure.db_sd,
                z_score: measure.z_score,
                db_n: measure.db_n,
            })
        })
        .collect()
}

/// Build a score vector from a `SpeakerKideval` for raw positional comparison.
///
/// Returns a vector with scores placed at their database column positions.
/// Unused positions are filled with 0.0.
pub fn speaker_to_score_vector(speaker: &SpeakerKideval) -> Vec<f64> {
    let max_col = MAPPINGS.iter().map(|m| m.col_index).max().unwrap_or(0);
    let mut scores = vec![0.0; max_col + 1];
    for m in MAPPINGS {
        scores[m.col_index] = (m.extract)(speaker);
    }
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_vector_roundtrip() {
        let speaker = SpeakerKideval {
            speaker: "CHI".to_owned(),
            utterances: 55,
            total_words: 94,
            ndw: 38,
            mlu_words: 1.71,
            mlu_morphemes: 1.87,
            vocd_score: 20.23,
            dss_score: 3.5,
            ipsyn_score: 33,
            word_errors: 2,
            ..Default::default()
        };
        let vec = speaker_to_score_vector(&speaker);
        assert!((vec[col::TOTAL_UTTS] - 55.0).abs() < f64::EPSILON);
        assert!((vec[col::FREQ_TOKENS] - 94.0).abs() < f64::EPSILON);
        assert!((vec[col::NDW] - 38.0).abs() < f64::EPSILON);
        assert!((vec[col::MLU_WORDS] - 1.71).abs() < 1e-10);
        assert!((vec[col::VOCD] - 20.23).abs() < 1e-10);
        assert!((vec[col::DSS] - 3.5).abs() < 1e-10);
        assert!((vec[col::IPSYN_TOTAL] - 33.0).abs() < f64::EPSILON);
    }

    #[test]
    fn mapping_with_empty_comparison() {
        let speaker = SpeakerKideval::default();
        let comparison = ComparisonResult {
            measures: Vec::new(),
            matched_entries: 0,
        };
        let mapped = map_kideval_comparison(&speaker, &comparison);
        // No measures in comparison → no mapped results
        assert!(mapped.is_empty());
    }
}
