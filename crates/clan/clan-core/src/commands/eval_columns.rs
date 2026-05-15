//! Column mapping between Eval `.cut` database scores and [`SpeakerEval`] fields.
//!
//! The Eval `.cut` database stores ~34 fixed metrics per gem group followed by
//! word frequency rankings. This module defines the fixed column positions and
//! provides conversion between `SpeakerEval` and the positional score vectors
//! used by the comparison engine.
//!
//! Column order is defined by `retrieveTS()` in `eval.cpp` (CLAN C source).
//!
//! # Differences from CLAN
//!
//! This module has no direct CLAN equivalent — CLAN embeds column mapping
//! inline in `eval.cpp`. Extracting it as a separate module enables reuse
//! by the VS Code extension and other API consumers.

use super::eval::SpeakerEval;
use crate::database::ComparisonResult;
use serde::Serialize;

/// Named column indices in the Eval `.cut` database.
///
/// These match the `retrieveTS()` read order in CLAN's `eval.cpp`.
pub mod col {
    /// Time duration in seconds.
    pub const TIME: usize = 0;
    /// NDW (number of different words / speaker word count).
    pub const NDW: usize = 1;
    /// Frequency tokens (total words).
    pub const FREQ_TOKENS: usize = 2;
    // 3: CUR (content units ratio)
    // 4: nounsNV (nouns for N/V ratio)
    // 5: verbsNV (verbs for N/V ratio)
    /// MLU words (sum, not average — divide by mluUtt for MLU).
    pub const MLU_WORDS_SUM: usize = 6;
    /// MLU morphemes (sum, not average).
    pub const MLU_MORF_SUM: usize = 7;
    /// Total utterances.
    pub const TOTAL_UTTS: usize = 8;
    /// MLU utterance count.
    pub const MLU_UTTS: usize = 9;
    /// Word errors.
    pub const WORD_ERRORS: usize = 10;
    /// Utterance errors.
    pub const UTT_ERRORS: usize = 11;
    /// Total morphemes on %mor tier.
    pub const MOR_TOTAL: usize = 12;
    // 13: density (lexical density)
    /// Nouns.
    pub const NOUNS: usize = 14;
    /// Verbs.
    pub const VERBS: usize = 15;
    /// Auxiliaries.
    pub const AUX: usize = 16;
    /// Modals.
    pub const MODALS: usize = 17;
    /// Prepositions.
    pub const PREP: usize = 18;
    /// Adjectives.
    pub const ADJ: usize = 19;
    /// Adverbs.
    pub const ADV: usize = 20;
    /// Conjunctions.
    pub const CONJ: usize = 21;
    /// Pronouns.
    pub const PRON: usize = 22;
    /// Determiners.
    pub const DET: usize = 23;
    // 24: thrS (3rd person -s)
    // 25: thrnS (3rd person non-s)
    /// Past tense.
    pub const PAST: usize = 26;
    /// Past participle.
    pub const PAST_PARTICIPLE: usize = 27;
    /// Plurals.
    pub const PLURALS: usize = 28;
    /// Present participle.
    pub const PRESENT_PARTICIPLE: usize = 29;
    /// Open class words count.
    pub const OPEN_CLASS: usize = 30;
    /// Closed class words count.
    pub const CLOSED_CLASS: usize = 31;
    // 32: retracings
    // 33: repetitions
}

/// A named comparison for a single Eval measure.
#[derive(Debug, Clone, Serialize)]
pub struct EvalMeasureComparison {
    /// Human-readable name of the measure (e.g. "MLU (words)").
    pub label: &'static str,
    /// The speaker's observed score.
    pub score: f64,
    /// Database mean for this measure.
    pub db_mean: f64,
    /// Database standard deviation.
    pub db_sd: f64,
    /// Z-score relative to the database, if SD > 0.
    pub z_score: Option<f64>,
    /// Number of database entries used for comparison.
    pub db_n: usize,
}

/// Mapping entry: which `.cut` column corresponds to which speaker field.
struct ColumnMapping {
    label: &'static str,
    col_index: usize,
    extract: fn(&SpeakerEval) -> f64,
}

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
        col_index: col::MLU_WORDS_SUM,
        extract: |s| s.mlu_words,
    },
    ColumnMapping {
        label: "MLU (morphemes)",
        col_index: col::MLU_MORF_SUM,
        extract: |s| s.mlu_morphemes,
    },
    ColumnMapping {
        label: "Total morphemes",
        col_index: col::MOR_TOTAL,
        extract: |s| s.total_morphemes as f64,
    },
    ColumnMapping {
        label: "Nouns",
        col_index: col::NOUNS,
        extract: |s| s.nouns as f64,
    },
    ColumnMapping {
        label: "Verbs",
        col_index: col::VERBS,
        extract: |s| s.verbs as f64,
    },
    ColumnMapping {
        label: "Auxiliaries",
        col_index: col::AUX,
        extract: |s| s.auxiliaries as f64,
    },
    ColumnMapping {
        label: "Modals",
        col_index: col::MODALS,
        extract: |s| s.modals as f64,
    },
    ColumnMapping {
        label: "Prepositions",
        col_index: col::PREP,
        extract: |s| s.prepositions as f64,
    },
    ColumnMapping {
        label: "Adjectives",
        col_index: col::ADJ,
        extract: |s| s.adjectives as f64,
    },
    ColumnMapping {
        label: "Adverbs",
        col_index: col::ADV,
        extract: |s| s.adverbs as f64,
    },
    ColumnMapping {
        label: "Conjunctions",
        col_index: col::CONJ,
        extract: |s| s.conjunctions as f64,
    },
    ColumnMapping {
        label: "Pronouns",
        col_index: col::PRON,
        extract: |s| s.pronouns as f64,
    },
    ColumnMapping {
        label: "Determiners",
        col_index: col::DET,
        extract: |s| s.determiners as f64,
    },
    ColumnMapping {
        label: "Plurals",
        col_index: col::PLURALS,
        extract: |s| s.plurals as f64,
    },
    ColumnMapping {
        label: "Past tense",
        col_index: col::PAST,
        extract: |s| s.past_tense as f64,
    },
    ColumnMapping {
        label: "Present participle",
        col_index: col::PRESENT_PARTICIPLE,
        extract: |s| s.present_participle as f64,
    },
    ColumnMapping {
        label: "Past participle",
        col_index: col::PAST_PARTICIPLE,
        extract: |s| s.past_participle as f64,
    },
    ColumnMapping {
        label: "Word errors",
        col_index: col::WORD_ERRORS,
        extract: |s| s.word_errors as f64,
    },
];

/// Extract the scores from a `SpeakerEval` that have database column mappings,
/// and produce named comparisons from a [`ComparisonResult`].
pub fn map_eval_comparison(
    speaker: &SpeakerEval,
    comparison: &ComparisonResult,
) -> Vec<EvalMeasureComparison> {
    MAPPINGS
        .iter()
        .filter_map(|m| {
            let measure = comparison.measures.get(m.col_index)?;
            Some(EvalMeasureComparison {
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

/// Build a score vector from a `SpeakerEval` for raw positional comparison.
pub fn speaker_to_score_vector(speaker: &SpeakerEval) -> Vec<f64> {
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
    fn score_vector_places_values_correctly() {
        let speaker = SpeakerEval {
            speaker: "PAR".to_owned(),
            utterances: 42,
            total_words: 120,
            ndw: 55,
            nouns: 18,
            verbs: 15,
            word_errors: 3,
            ..Default::default()
        };
        let vec = speaker_to_score_vector(&speaker);
        assert!((vec[col::TOTAL_UTTS] - 42.0).abs() < f64::EPSILON);
        assert!((vec[col::FREQ_TOKENS] - 120.0).abs() < f64::EPSILON);
        assert!((vec[col::NDW] - 55.0).abs() < f64::EPSILON);
        assert!((vec[col::NOUNS] - 18.0).abs() < f64::EPSILON);
        assert!((vec[col::WORD_ERRORS] - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn mapping_with_empty_comparison() {
        let speaker = SpeakerEval::default();
        let comparison = ComparisonResult {
            measures: Vec::new(),
            matched_entries: 0,
        };
        let mapped = map_eval_comparison(&speaker, &comparison);
        assert!(mapped.is_empty());
    }
}
