//! EVAL — Language sample evaluation for clinical analysis.
//!
//! Comprehensive morphosyntactic analysis computing lexical diversity,
//! grammatical category counts, error rates, and MLU. EVAL was originally
//! designed for clinical evaluation of adult aphasic speech samples
//! (Saffran, Berndt & Schwartz, 1989) and produces a detailed profile
//! of morphosyntactic abilities.
//!
//! Metrics include: utterance count, total words, NDW (number of different
//! words), TTR (type-token ratio), MLU in words and morphemes, per-category
//! POS counts (nouns, verbs, auxiliaries, modals, prepositions, adjectives,
//! adverbs, conjunctions, determiners, pronouns), inflectional morphology
//! counts (plurals, past tense, present/past participle), word-level error
//! counts (`[*]`), and open/closed class ratio.
//!
//! Requires a `%mor` dependent tier for morpheme-level metrics. Word-level
//! metrics are computed from the main tier regardless.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc87376473)
//! for the original EVAL command specification.
//!
//! # Differences from CLAN
//!
//! - Word and morpheme identification uses AST-based `is_countable_word()`
//!   and typed POS categories instead of CLAN's string-prefix matching.
//! - Error counts (`[*]`) are extracted from parsed AST annotations rather
//!   than raw text pattern matching.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::mor::{self, MorPosCount};
use crate::framework::{
    AnalysisCommand, AnalysisResult, AnalysisScore, CommandOutput, FileContext, MorphemeCount,
    OutputFormat, POSCount, Section, TableRow, TypeCount, UtteranceCount, WordCount,
    count_main_scoped_errors, countable_words,
};

/// Whether to use standard or dialect (DementiaBank) norms.
///
/// EVAL uses AphasiaBank norms by default; EVAL-D uses DementiaBank norms.
/// The only difference is which `.cut` database files are selected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EvalVariant {
    /// Standard EVAL (AphasiaBank norms).
    #[default]
    Standard,
    /// EVAL-D (DementiaBank protocol norms).
    Dialect,
}

/// Configuration for the EVAL command.
#[derive(Debug, Clone, Default)]
pub struct EvalConfig {
    /// Path to optional database file for comparison stats.
    pub database_path: Option<std::path::PathBuf>,
    /// Filter criteria for selecting comparison entries from the database.
    pub database_filter: Option<crate::database::DatabaseFilter>,
    /// Standard EVAL vs EVAL-D (dialect/DementiaBank variant).
    pub variant: EvalVariant,
}

/// Per-speaker evaluation metrics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SpeakerEval {
    /// Speaker identifier.
    pub speaker: String,
    /// Number of utterances.
    pub utterances: UtteranceCount,
    /// Total words (tokens).
    pub total_words: WordCount,
    /// Number of different words (types).
    pub ndw: TypeCount,
    /// Type-token ratio.
    pub ttr: AnalysisScore,

    // Morphological category counts
    /// Nouns.
    pub nouns: POSCount,
    /// Verbs (all types).
    pub verbs: POSCount,
    /// Auxiliary verbs.
    pub auxiliaries: POSCount,
    /// Modal verbs.
    pub modals: POSCount,
    /// Prepositions.
    pub prepositions: POSCount,
    /// Adjectives.
    pub adjectives: POSCount,
    /// Adverbs.
    pub adverbs: POSCount,
    /// Conjunctions.
    pub conjunctions: POSCount,
    /// Determiners.
    pub determiners: POSCount,
    /// Pronouns.
    pub pronouns: POSCount,
    /// Plurals.
    pub plurals: POSCount,
    /// Past tense.
    pub past_tense: POSCount,
    /// Present participle (-ing).
    pub present_participle: POSCount,
    /// Past participle.
    pub past_participle: POSCount,

    // Error counts
    /// Word-level errors `[*]`.
    pub word_errors: POSCount,
    /// Utterance-level errors (from postcodes).
    pub utterance_errors: UtteranceCount,

    // Derived metrics
    /// Mean length of utterance (words).
    pub mlu_words: AnalysisScore,
    /// Mean length of utterance (morphemes).
    pub mlu_morphemes: AnalysisScore,
    /// Total morphemes.
    pub total_morphemes: MorphemeCount,
    /// Open-closed ratio (content words / function words).
    pub open_closed_ratio: AnalysisScore,
}

/// Typed output for the EVAL command.
#[derive(Debug, Clone, Serialize)]
pub struct EvalResult {
    /// Per-speaker evaluation data.
    pub speakers: Vec<SpeakerEval>,
    /// Per-speaker normative comparisons (parallel to `speakers`).
    /// Present only when a database was provided in config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparisons: Option<Vec<Vec<super::eval_columns::EvalMeasureComparison>>>,
}

impl EvalResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("eval");
        for (idx, sp) in self.speakers.iter().enumerate() {
            let comparison = self.comparisons.as_ref().and_then(|c| c.get(idx));

            if let Some(cmp) = comparison {
                let headers = vec![
                    "Measure".to_owned(),
                    "Score".to_owned(),
                    "DB Mean".to_owned(),
                    "DB SD".to_owned(),
                    "Z-Score".to_owned(),
                    "N".to_owned(),
                ];
                let rows = cmp
                    .iter()
                    .map(|m| TableRow {
                        values: vec![
                            m.label.to_owned(),
                            format!("{:.2}", m.score),
                            format!("{:.2}", m.db_mean),
                            format!("{:.2}", m.db_sd),
                            m.z_score
                                .map(|z| format!("{z:+.2}"))
                                .unwrap_or_else(|| "—".to_owned()),
                            m.db_n.to_string(),
                        ],
                    })
                    .collect();
                let section =
                    Section::with_table(format!("Speaker: {}", sp.speaker), headers, rows);
                result.add_section(section);
            } else {
                let rows = vec![
                    TableRow {
                        values: vec!["Utterances".to_owned(), sp.utterances.to_string()],
                    },
                    TableRow {
                        values: vec!["Total words".to_owned(), sp.total_words.to_string()],
                    },
                    TableRow {
                        values: vec!["NDW (types)".to_owned(), sp.ndw.to_string()],
                    },
                    TableRow {
                        values: vec!["TTR".to_owned(), format!("{:.3}", sp.ttr)],
                    },
                    TableRow {
                        values: vec!["MLU (words)".to_owned(), format!("{:.2}", sp.mlu_words)],
                    },
                    TableRow {
                        values: vec![
                            "MLU (morphemes)".to_owned(),
                            format!("{:.2}", sp.mlu_morphemes),
                        ],
                    },
                    TableRow {
                        values: vec!["Nouns".to_owned(), sp.nouns.to_string()],
                    },
                    TableRow {
                        values: vec!["Verbs".to_owned(), sp.verbs.to_string()],
                    },
                    TableRow {
                        values: vec!["Auxiliaries".to_owned(), sp.auxiliaries.to_string()],
                    },
                    TableRow {
                        values: vec!["Modals".to_owned(), sp.modals.to_string()],
                    },
                    TableRow {
                        values: vec!["Prepositions".to_owned(), sp.prepositions.to_string()],
                    },
                    TableRow {
                        values: vec!["Adjectives".to_owned(), sp.adjectives.to_string()],
                    },
                    TableRow {
                        values: vec!["Adverbs".to_owned(), sp.adverbs.to_string()],
                    },
                    TableRow {
                        values: vec!["Conjunctions".to_owned(), sp.conjunctions.to_string()],
                    },
                    TableRow {
                        values: vec!["Determiners".to_owned(), sp.determiners.to_string()],
                    },
                    TableRow {
                        values: vec!["Pronouns".to_owned(), sp.pronouns.to_string()],
                    },
                    TableRow {
                        values: vec!["Word errors".to_owned(), sp.word_errors.to_string()],
                    },
                ];
                let section = Section::with_table(
                    format!("Speaker: {}", sp.speaker),
                    vec!["Metric".to_owned(), "Value".to_owned()],
                    rows,
                );
                result.add_section(section);
            }
        }
        result
    }
}

impl CommandOutput for EvalResult {
    /// Render evaluation metrics as a human-readable text table.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render evaluation metrics in CLAN-compatible format.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for sp in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", sp.speaker));
            out.push_str(&format!("  Utterances:       {}\n", sp.utterances));
            out.push_str(&format!("  Total words:      {}\n", sp.total_words));
            out.push_str(&format!("  NDW:              {}\n", sp.ndw));
            out.push_str(&format!("  TTR:              {:.3}\n", sp.ttr));
            out.push_str(&format!("  MLU (words):      {:.2}\n", sp.mlu_words));
            out.push_str(&format!("  MLU (morphemes):  {:.2}\n", sp.mlu_morphemes));
            out.push_str(&format!("  Nouns:            {}\n", sp.nouns));
            out.push_str(&format!("  Verbs:            {}\n", sp.verbs));
            out.push_str(&format!("  Auxiliaries:      {}\n", sp.auxiliaries));
            out.push_str(&format!("  Modals:           {}\n", sp.modals));
            out.push_str(&format!("  Prepositions:     {}\n", sp.prepositions));
            out.push_str(&format!("  Adjectives:       {}\n", sp.adjectives));
            out.push_str(&format!("  Adverbs:          {}\n", sp.adverbs));
            out.push_str(&format!("  Conjunctions:     {}\n", sp.conjunctions));
            out.push_str(&format!("  Determiners:      {}\n", sp.determiners));
            out.push_str(&format!("  Pronouns:         {}\n", sp.pronouns));
            out.push_str(&format!("  Word errors:      {}\n\n", sp.word_errors));
        }
        out
    }
}

/// Per-speaker accumulator during processing (internal).
///
/// Collects raw counts and per-utterance word/morpheme totals that are
/// used by [`EvalCommand::finalize`] to compute derived metrics (TTR, MLU,
/// open/closed ratio).
#[derive(Debug, Default)]
pub struct SpeakerAccum {
    /// Word counts per utterance.
    pub words_per_utt: Vec<WordCount>,
    /// Morphemes per utterance.
    pub morphemes_per_utt: Vec<MorphemeCount>,
    /// Unique words (for NDW).
    pub unique_words: BTreeSet<String>,
    /// Total words.
    pub total_words: WordCount,
    /// Morphological category counts (shared with KIDEVAL).
    pub pos: MorPosCount,
    /// Error counts.
    pub word_errors: u64,
    /// Utterance errors.
    pub utterance_errors: u64,
}

/// Accumulated state for EVAL.
#[derive(Debug, Default)]
pub struct EvalState {
    /// Per-speaker accumulator.
    speakers: BTreeMap<String, SpeakerAccum>,
}

/// EVAL command implementation.
///
/// Processes each utterance by counting words on the main tier and
/// classifying `%mor` tokens into grammatical categories. Derived metrics
/// (TTR, MLU, open/closed ratio) are computed during finalization.
pub struct EvalCommand {
    config: EvalConfig,
    database: Option<crate::database::ParsedDatabase>,
}

impl EvalCommand {
    /// Create a new EVAL command, optionally loading a normative database.
    pub fn new(config: EvalConfig) -> Self {
        let database = config.database_path.as_ref().and_then(|path| {
            crate::database::parse_database(path)
                .inspect_err(|e| tracing::warn!("Failed to load eval database: {e}"))
                .ok()
        });
        Self { config, database }
    }
}

impl AnalysisCommand for EvalCommand {
    type Config = EvalConfig;
    type State = EvalState;
    type Output = EvalResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();
        let accum = state.speakers.entry(speaker).or_default();

        accum.word_errors += count_main_scoped_errors(&utterance.main.content.content);

        let mut word_count = 0u64;
        for word in countable_words(&utterance.main.content.content) {
            accum
                .unique_words
                .insert(word.cleaned_text().to_lowercase());
            word_count += 1;
        }
        accum.total_words += word_count;
        accum.words_per_utt.push(word_count);

        // Process %mor tier using typed MorTier items
        let mut morpheme_count = 0u64;
        if let Some(mor_tier) = mor::extract_mor_tier(utterance) {
            for item in mor_tier.items().iter() {
                mor::classify_mor_item(item, &mut accum.pos);
                morpheme_count += mor::count_morphemes_typed(item);
            }
        }
        accum.morphemes_per_utt.push(morpheme_count);
    }

    fn finalize(&self, state: Self::State) -> EvalResult {
        let speakers: Vec<_> = state
            .speakers
            .into_iter()
            .map(|(speaker, accum)| {
                let utterances = accum.words_per_utt.len() as u64;
                let total_words = accum.total_words;
                let ndw = accum.unique_words.len() as u64;
                let ttr = if total_words > 0 {
                    ndw as f64 / total_words as f64
                } else {
                    0.0
                };
                let mlu_words = if utterances > 0 {
                    total_words as f64 / utterances as f64
                } else {
                    0.0
                };
                let total_morphemes: u64 = accum.morphemes_per_utt.iter().sum();
                let mlu_morphemes = if utterances > 0 {
                    total_morphemes as f64 / utterances as f64
                } else {
                    0.0
                };

                let content_words =
                    accum.pos.nouns + accum.pos.verbs + accum.pos.adjectives + accum.pos.adverbs;
                let function_words = accum.pos.determiners
                    + accum.pos.prepositions
                    + accum.pos.conjunctions
                    + accum.pos.pronouns;
                let open_closed_ratio = if function_words > 0 {
                    content_words as f64 / function_words as f64
                } else {
                    0.0
                };

                SpeakerEval {
                    speaker,
                    utterances,
                    total_words,
                    ndw,
                    ttr,
                    nouns: accum.pos.nouns,
                    verbs: accum.pos.verbs,
                    auxiliaries: accum.pos.auxiliaries,
                    modals: accum.pos.modals,
                    prepositions: accum.pos.prepositions,
                    adjectives: accum.pos.adjectives,
                    adverbs: accum.pos.adverbs,
                    conjunctions: accum.pos.conjunctions,
                    determiners: accum.pos.determiners,
                    pronouns: accum.pos.pronouns,
                    plurals: accum.pos.plurals,
                    past_tense: accum.pos.past_tense,
                    present_participle: accum.pos.present_participle,
                    past_participle: accum.pos.past_participle,
                    word_errors: accum.word_errors,
                    utterance_errors: accum.utterance_errors,
                    mlu_words,
                    mlu_morphemes,
                    total_morphemes,
                    open_closed_ratio,
                }
            })
            .collect();

        // Perform database comparison if configured
        let comparisons = self.database.as_ref().map(|db| {
            let filter = self.config.database_filter.clone().unwrap_or_default();
            let matched = filter.apply(&db.entries);
            speakers
                .iter()
                .map(|sp| {
                    let score_vec = crate::commands::eval_columns::speaker_to_score_vector(sp);
                    let comparison = crate::database::compare_to_norms(&score_vec, &matched);
                    crate::commands::eval_columns::map_eval_comparison(sp, &comparison)
                })
                .collect()
        });

        EvalResult {
            speakers,
            comparisons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_empty() {
        let cmd = EvalCommand::new(EvalConfig::default());
        let state = EvalState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }
}
