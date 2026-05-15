//! KIDEVAL — Child language evaluation (combined assessment).
//!
//! Produces a comprehensive child language evaluation report by combining
//! multiple analysis methods into a single per-speaker summary:
//!
//! - **MLU** (words and morphemes) from main tier and `%mor`
//! - **NDW / TTR** (number of different words / type-token ratio)
//! - **DSS** (Developmental Sentence Scoring) from `%mor`
//! - **VOCD** (vocabulary diversity D statistic)
//! - **IPSYN** (Index of Productive Syntax)
//! - **Morphological category counts** (nouns, verbs, auxiliaries, etc.)
//! - **Word error counts** (`[*]` markers)
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409281)
//! for the original KIDEVAL command specification.
//!
//! # Differences from CLAN
//!
//! - VOCD uses a simplified TTR-based D estimate rather than the full
//!   bootstrap sampling approach (see [`vocd`](super::vocd) for the full
//!   algorithm).
//! - IPSYN uses the built-in simplified rule subset unless a custom rules
//!   file is provided.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use talkbank_model::{Mor, Utterance};

use crate::commands::dss::{DssRuleSet, is_complete_sentence, score_utterance as dss_score};
use crate::framework::mor::{self, MorPosCount};
use crate::framework::{
    AnalysisCommand, AnalysisResult, AnalysisScore, CommandOutput, FileContext, MorphemeCount,
    OutputFormat, POSCount, ScorePoints, Section, TableRow, TransformError, TypeCount,
    UtteranceCount, WordCount, count_main_scoped_errors, countable_words, spoken_main_text,
};

/// Configuration for the KIDEVAL command.
#[derive(Debug, Clone)]
pub struct KidevalConfig {
    /// Path to DSS rules file.
    pub dss_rules_path: Option<std::path::PathBuf>,
    /// Path to IPSYN rules file.
    pub ipsyn_rules_path: Option<std::path::PathBuf>,
    /// Maximum utterances for DSS (default: 50).
    pub dss_max_utterances: crate::framework::UtteranceLimit,
    /// Maximum utterances for IPSYN (default: 100).
    pub ipsyn_max_utterances: crate::framework::UtteranceLimit,
    /// Path to a normative database `.cut` file for comparison.
    pub database_path: Option<std::path::PathBuf>,
    /// Filter criteria for selecting comparison entries from the database.
    pub database_filter: Option<crate::database::DatabaseFilter>,
}

impl Default for KidevalConfig {
    fn default() -> Self {
        Self {
            dss_rules_path: None,
            ipsyn_rules_path: None,
            dss_max_utterances: crate::framework::UtteranceLimit::new(50),
            ipsyn_max_utterances: crate::framework::UtteranceLimit::new(100),
            database_path: None,
            database_filter: None,
        }
    }
}

/// Per-speaker combined evaluation metrics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SpeakerKideval {
    /// Speaker identifier.
    pub speaker: String,
    /// Number of utterances analyzed.
    pub utterances: UtteranceCount,
    /// Total words (tokens).
    pub total_words: WordCount,
    /// Number of different words (NDW).
    pub ndw: TypeCount,
    /// Type-token ratio (TTR).
    pub ttr: AnalysisScore,
    /// MLU in words.
    pub mlu_words: AnalysisScore,
    /// MLU in morphemes.
    pub mlu_morphemes: AnalysisScore,

    // Morphological category counts
    /// Nouns.
    pub nouns: POSCount,
    /// Verbs.
    pub verbs: POSCount,
    /// Auxiliaries.
    pub auxiliaries: POSCount,
    /// Modals.
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

    // Combined scores
    /// DSS score (from developmental sentence scoring).
    pub dss_score: AnalysisScore,
    /// VOCD score (from vocabulary diversity — uses existing vocd command).
    pub vocd_score: AnalysisScore,
    /// IPSYN score (from productive syntax).
    pub ipsyn_score: ScorePoints,

    /// Word-level errors.
    pub word_errors: POSCount,
}

/// Typed output for the KIDEVAL command.
#[derive(Debug, Clone, Serialize)]
pub struct KidevalResult {
    /// Per-speaker combined results.
    pub speakers: Vec<SpeakerKideval>,
    /// Per-speaker normative comparisons (parallel to `speakers`).
    /// Present only when a database was provided in config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparisons: Option<Vec<Vec<super::kideval_columns::KidevalMeasureComparison>>>,
}

impl KidevalResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("kideval");
        for (i, sp) in self.speakers.iter().enumerate() {
            let has_comparison = self
                .comparisons
                .as_ref()
                .is_some_and(|c| !c.get(i).is_some_and(|v| v.is_empty()));

            let (columns, rows) = if has_comparison {
                // Control-flow invariant: `has_comparison` is true
                // only when `self.comparisons.as_ref().is_some_and(...)`
                // holds, which proves `self.comparisons` is `Some`.
                #[allow(clippy::unwrap_used)]
                let comps = &self.comparisons.as_ref().unwrap()[i];
                let cols = vec![
                    "Metric".to_owned(),
                    "Score".to_owned(),
                    "DB Mean".to_owned(),
                    "DB SD".to_owned(),
                    "Z-Score".to_owned(),
                    "N".to_owned(),
                ];
                let rows: Vec<TableRow> = comps
                    .iter()
                    .map(|c| TableRow {
                        values: vec![
                            c.label.to_owned(),
                            format!("{:.2}", c.score),
                            format!("{:.2}", c.db_mean),
                            format!("{:.2}", c.db_sd),
                            c.z_score
                                .map(|z| format!("{z:+.2}"))
                                .unwrap_or_else(|| "N/A".to_owned()),
                            c.db_n.to_string(),
                        ],
                    })
                    .collect();
                (cols, rows)
            } else {
                let cols = vec!["Metric".to_owned(), "Value".to_owned()];
                let rows = vec![
                    TableRow {
                        values: vec!["Utterances".to_owned(), sp.utterances.to_string()],
                    },
                    TableRow {
                        values: vec!["Total words".to_owned(), sp.total_words.to_string()],
                    },
                    TableRow {
                        values: vec!["NDW".to_owned(), sp.ndw.to_string()],
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
                        values: vec!["DSS score".to_owned(), format!("{:.2}", sp.dss_score)],
                    },
                    TableRow {
                        values: vec!["IPSYN score".to_owned(), sp.ipsyn_score.to_string()],
                    },
                    TableRow {
                        values: vec!["Nouns".to_owned(), sp.nouns.to_string()],
                    },
                    TableRow {
                        values: vec!["Verbs".to_owned(), sp.verbs.to_string()],
                    },
                    TableRow {
                        values: vec!["Pronouns".to_owned(), sp.pronouns.to_string()],
                    },
                    TableRow {
                        values: vec!["Word errors".to_owned(), sp.word_errors.to_string()],
                    },
                ];
                (cols, rows)
            };

            let section = Section::with_table(format!("Speaker: {}", sp.speaker), columns, rows);
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for KidevalResult {
    /// Render per-speaker metric/value table.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible fixed-width summary per speaker.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for sp in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", sp.speaker));
            out.push_str(&format!("  Utterances:      {}\n", sp.utterances));
            out.push_str(&format!("  Total words:     {}\n", sp.total_words));
            out.push_str(&format!("  NDW:             {}\n", sp.ndw));
            out.push_str(&format!("  TTR:             {:.3}\n", sp.ttr));
            out.push_str(&format!("  MLU (words):     {:.2}\n", sp.mlu_words));
            out.push_str(&format!("  MLU (morphemes): {:.2}\n", sp.mlu_morphemes));
            out.push_str(&format!("  DSS:             {:.2}\n", sp.dss_score));
            out.push_str(&format!("  IPSYN:           {}\n", sp.ipsyn_score));
            out.push_str(&format!("  Nouns:           {}\n", sp.nouns));
            out.push_str(&format!("  Verbs:           {}\n", sp.verbs));
            out.push_str(&format!("  Pronouns:        {}\n", sp.pronouns));
            out.push_str(&format!("  Word errors:     {}\n\n", sp.word_errors));
        }
        out
    }
}

/// Per-speaker accumulator.
#[derive(Debug, Default)]
struct SpeakerAccum {
    /// Word counts per utterance.
    words_per_utt: Vec<WordCount>,
    /// Morpheme counts per utterance.
    morphemes_per_utt: Vec<MorphemeCount>,
    /// Unique words.
    unique_words: BTreeSet<String>,
    /// Total words.
    total_words: WordCount,
    /// Typed %mor items per utterance for DSS/IPSYN scoring.
    mor_items: Vec<Vec<Mor>>,
    /// Main tier texts for display.
    main_texts: Vec<String>,
    /// Morphological category counts (shared with EVAL).
    pos: MorPosCount,
    /// Error counts.
    word_errors: u64,
    /// All word tokens (for VOCD).
    word_tokens: Vec<String>,
}

/// Accumulated state for KIDEVAL.
#[derive(Debug, Default)]
pub struct KidevalState {
    /// Per-speaker accumulator.
    speakers: BTreeMap<String, SpeakerAccum>,
}

/// KIDEVAL command implementation.
///
/// Accumulates per-speaker word counts, morphological category counts,
/// and `%mor` tier texts during utterance processing. At finalization,
/// computes MLU, TTR, DSS, IPSYN, and VOCD scores from the accumulated data.
/// Optionally compares results against a normative database.
pub struct KidevalCommand {
    config: KidevalConfig,
    dss_rules: DssRuleSet,
    ipsyn_rules: crate::commands::ipsyn::IpsynRuleSet,
    database: Option<crate::database::ParsedDatabase>,
}

impl KidevalCommand {
    /// Create a new KIDEVAL command.
    pub fn new(config: KidevalConfig) -> Result<Self, TransformError> {
        let dss_rules = if let Some(ref path) = config.dss_rules_path {
            crate::commands::dss::load_dss_rules(path)?
        } else {
            DssRuleSet::default()
        };
        let ipsyn_rules = if let Some(ref path) = config.ipsyn_rules_path {
            crate::commands::ipsyn::load_ipsyn_rules(path)?
        } else {
            crate::commands::ipsyn::IpsynRuleSet::default()
        };
        let database = if let Some(ref path) = config.database_path {
            Some(crate::database::parse_database(path)?)
        } else {
            None
        };
        Ok(Self {
            config,
            dss_rules,
            ipsyn_rules,
            database,
        })
    }
}

/// Simplified VOCD computation using the TTR-to-D inverse formula.
///
/// Returns 0.0 if fewer than 50 tokens are available (insufficient data).
/// For the full bootstrap-based VOCD algorithm, see [`VocdCommand`](super::vocd::VocdCommand).
fn compute_vocd_score(tokens: &[String]) -> f64 {
    if tokens.len() < 50 {
        return 0.0;
    }
    // Use a simplified TTR-based D estimate
    let types: BTreeSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
    let ttr = types.len() as f64 / tokens.len() as f64;
    // Approximate D from TTR using the inverse of the theoretical TTR equation
    // TTR ≈ D/N * ((1 + 2N/D)^0.5 - 1)
    // For a rough estimate: D ≈ N * TTR² / (1 - TTR)
    let n = tokens.len() as f64;
    if ttr >= 1.0 {
        return 120.0; // cap
    }
    let d_estimate = n * ttr * ttr / (1.0 - ttr);
    d_estimate.clamp(0.0, 120.0)
}

/// Compute IPSYN score from typed `%mor` items using the provided rule set.
///
/// Analyzes at most `max_utts` utterances. Each rule can score at most
/// 2 points (one per distinct matching utterance).
fn compute_ipsyn_score(
    mor_items: &[Vec<Mor>],
    rules: &crate::commands::ipsyn::IpsynRuleSet,
    max_utts: usize,
) -> u32 {
    let max = max_utts.min(mor_items.len());
    let analyze = &mor_items[..max];
    let mut total = 0u32;

    for rule in &rules.rules {
        let mut matches = 0u32;
        for items in analyze {
            if crate::commands::ipsyn::rule_matches(items, rule) {
                matches += 1;
                if matches >= 2 {
                    break;
                }
            }
        }
        total += matches.min(2);
    }
    total
}

impl AnalysisCommand for KidevalCommand {
    type Config = KidevalConfig;
    type State = KidevalState;
    type Output = KidevalResult;

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
            let normalized = word.cleaned_text().to_lowercase();
            accum.unique_words.insert(normalized.clone());
            accum.word_tokens.push(normalized);
            word_count += 1;
        }
        accum.total_words += word_count;
        accum.words_per_utt.push(word_count);
        accum.main_texts.push(spoken_main_text(&utterance.main));

        // Process %mor tier using typed MorTier items
        let mut morpheme_count = 0u64;
        if let Some(mor_tier) = mor::extract_mor_tier(utterance) {
            for item in mor_tier.items().iter() {
                mor::classify_mor_item(item, &mut accum.pos);
                morpheme_count += mor::count_morphemes_typed(item);
            }
            accum.mor_items.push(mor_tier.items().to_vec());
        }
        accum.morphemes_per_utt.push(morpheme_count);
    }

    fn finalize(&self, state: Self::State) -> KidevalResult {
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

                // Compute DSS score
                let dss_max = self
                    .config
                    .dss_max_utterances
                    .get()
                    .min(accum.mor_items.len());
                let mut dss_total = 0u32;
                for items in accum.mor_items.iter().take(dss_max) {
                    let (_, total) = dss_score(items, &self.dss_rules);
                    dss_total += total + u32::from(is_complete_sentence(items));
                }
                let dss_score = if dss_max > 0 {
                    dss_total as f64 / dss_max as f64
                } else {
                    0.0
                };

                // Compute IPSYN score
                let ipsyn_score = compute_ipsyn_score(
                    &accum.mor_items,
                    &self.ipsyn_rules,
                    self.config.ipsyn_max_utterances.get(),
                );

                // Compute VOCD
                let vocd_score = compute_vocd_score(&accum.word_tokens);

                SpeakerKideval {
                    speaker,
                    utterances,
                    total_words,
                    ndw,
                    ttr,
                    mlu_words,
                    mlu_morphemes,
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
                    dss_score,
                    vocd_score,
                    ipsyn_score,
                    word_errors: accum.word_errors,
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
                    let score_vec = crate::commands::kideval_columns::speaker_to_score_vector(sp);
                    let comparison = crate::database::compare_to_norms(&score_vec, &matched);
                    crate::commands::kideval_columns::map_kideval_comparison(sp, &comparison)
                })
                .collect()
        });

        KidevalResult {
            speakers,
            comparisons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kideval_empty() {
        let cmd = KidevalCommand::new(KidevalConfig::default()).unwrap();
        let state = KidevalState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
        assert!(result.comparisons.is_none());
    }

    #[test]
    fn vocd_score_basic() {
        // Not enough tokens
        let short: Vec<String> = (0..10).map(|i| format!("word{i}")).collect();
        assert_eq!(compute_vocd_score(&short), 0.0);

        // Enough tokens with some repetition
        let mut tokens: Vec<String> = Vec::new();
        for i in 0..100 {
            tokens.push(format!("word{}", i % 30)); // 30 unique words in 100 tokens
        }
        let score = compute_vocd_score(&tokens);
        assert!(score > 0.0);
    }
}
