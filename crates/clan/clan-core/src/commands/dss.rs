//! DSS — Developmental Sentence Scoring.
//!
//! Assigns point values to utterances based on grammatical complexity,
//! using a configurable rule file that defines pattern-matching rules
//! for morphosyntactic categories. DSS is a clinical tool developed by
//! Laura Lee and Susan Canter for evaluating children's grammatical
//! development by scoring complete sentences on eight grammatical categories
//! (e.g., pronouns, verbs, negation, conjunctions, wh-questions).
//!
//! Scoring requires a `%mor` dependent tier on each utterance. Utterances
//! without `%mor` are silently skipped.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#DSS_Command)
//! for the original DSS command specification and the full rule set.
//!
//! # Differences from CLAN
//!
//! - The built-in default rules are a simplified subset of the canonical
//!   DSS rule set (10 categories). For full clinical scoring, supply a
//!   complete `.scr` rules file via `rules_path`.
//! - Sentence-point assignment uses a heuristic (presence of subject +
//!   verb POS tags) rather than full syntactic analysis.
//! - By default, up to 50 utterances per speaker are scored (configurable
//!   via `max_utterances`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use talkbank_model::{Mor, Utterance};

use crate::framework::mor;
use crate::framework::{
    AnalysisCommand, AnalysisResult, AnalysisScore, CommandOutput, FileContext, OutputFormat,
    ScorePoints, Section, TransformError, UtteranceCount, UtteranceLimit, spoken_main_text,
};

/// Configuration for the DSS command.
#[derive(Debug, Clone)]
pub struct DssConfig {
    /// Path to DSS rules file (.scr).
    pub rules_path: Option<PathBuf>,
    /// Maximum number of unique utterances to score (default: 50).
    pub max_utterances: UtteranceLimit,
}

impl Default for DssConfig {
    fn default() -> Self {
        Self {
            rules_path: None,
            max_utterances: UtteranceLimit::new(50),
        }
    }
}

/// A DSS rule: pattern on %mor tier → point value.
#[derive(Debug, Clone)]
pub struct DssRule {
    /// Rule category name (e.g., "indefinite_pronoun").
    pub category: String,
    /// POS/morpheme patterns to match (simplified: list of POS tags).
    pub patterns: Vec<String>,
    /// Point value awarded.
    pub points: u32,
}

/// A loaded DSS rule set.
///
/// Contains all scoring rules for DSS analysis. If no custom rules file
/// is provided, the default English rules are used.
#[derive(Debug, Clone)]
pub struct DssRuleSet {
    /// All rules, typically grouped by grammatical category.
    pub rules: Vec<DssRule>,
}

impl Default for DssRuleSet {
    fn default() -> Self {
        Self {
            rules: default_english_rules(),
        }
    }
}

/// Default English DSS rules (simplified version of the canonical rule set).
///
/// The full DSS has ~20 categories with hundreds of patterns across 8 developmental
/// levels. This provides the core categories to demonstrate the scoring framework.
fn default_english_rules() -> Vec<DssRule> {
    vec![
        // Indefinite pronouns / noun modifiers
        DssRule {
            category: "indefinite_pronouns".to_owned(),
            patterns: vec!["pro:indef".to_owned()],
            points: 1,
        },
        // Personal pronouns (1st/2nd person)
        DssRule {
            category: "personal_pronouns".to_owned(),
            patterns: vec!["pro:sub".to_owned(), "pro:obj".to_owned()],
            points: 1,
        },
        // Main verbs (uninflected)
        DssRule {
            category: "main_verbs".to_owned(),
            patterns: vec!["v".to_owned()],
            points: 1,
        },
        // Copula (is/am/are)
        DssRule {
            category: "copula".to_owned(),
            patterns: vec!["cop".to_owned()],
            points: 2,
        },
        // Auxiliary verbs
        DssRule {
            category: "auxiliaries".to_owned(),
            patterns: vec!["aux".to_owned()],
            points: 2,
        },
        // Past tense (-ed)
        DssRule {
            category: "past_tense".to_owned(),
            patterns: vec!["v-PAST".to_owned()],
            points: 2,
        },
        // Negatives
        DssRule {
            category: "negation".to_owned(),
            patterns: vec!["neg".to_owned()],
            points: 1,
        },
        // Conjunctions
        DssRule {
            category: "conjunctions".to_owned(),
            patterns: vec!["conj:coo".to_owned(), "conj:sub".to_owned()],
            points: 3,
        },
        // Interrogative reversals (wh-questions)
        DssRule {
            category: "wh_questions".to_owned(),
            patterns: vec!["pro:wh".to_owned(), "adv:wh".to_owned()],
            points: 2,
        },
        // Articles
        DssRule {
            category: "articles".to_owned(),
            patterns: vec!["det:art".to_owned()],
            points: 1,
        },
    ]
}

/// Load DSS rules from a `.scr` file.
///
/// The file format has one rule per section. Each section starts with a
/// header line `CATEGORY_NAME <points>` followed by one or more POS pattern
/// lines. Blank lines and lines starting with `#` are ignored.
///
/// # Format Example
///
/// ```text
/// # Pronouns
/// personal_pronouns 1
/// pro:sub
/// pro:obj
///
/// # Copula
/// copula 2
/// cop
/// ```
///
/// # Errors
///
/// Returns [`TransformError::Io`] if the file cannot be read.
pub fn load_dss_rules(path: &std::path::Path) -> Result<DssRuleSet, TransformError> {
    let content = std::fs::read_to_string(path).map_err(TransformError::Io)?;
    let mut rules = Vec::new();
    let mut current_category = String::new();
    let mut current_points = 0u32;
    let mut current_patterns = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check for category header: "CATEGORY_NAME points"
        if let Some((cat, pts_str)) = line.rsplit_once(' ')
            && let Ok(pts) = pts_str.parse::<u32>()
        {
            // Flush previous rule
            if !current_category.is_empty() && !current_patterns.is_empty() {
                rules.push(DssRule {
                    category: current_category.clone(),
                    patterns: current_patterns.clone(),
                    points: current_points,
                });
                current_patterns.clear();
            }
            current_category = cat.to_owned();
            current_points = pts;
            continue;
        }

        // Pattern line
        if !current_category.is_empty() {
            current_patterns.push(line.to_owned());
        }
    }

    // Flush last rule
    if !current_category.is_empty() && !current_patterns.is_empty() {
        rules.push(DssRule {
            category: current_category,
            patterns: current_patterns,
            points: current_points,
        });
    }

    Ok(DssRuleSet { rules })
}

/// Per-utterance DSS score.
#[derive(Debug, Clone, Serialize)]
pub struct UtteranceScore {
    /// Utterance index (1-based).
    pub index: usize,
    /// Utterance text (abbreviated).
    pub text: String,
    /// Points per category.
    pub category_points: BTreeMap<String, ScorePoints>,
    /// Total points for this utterance.
    pub total: ScorePoints,
    /// Whether this utterance is a complete sentence (awards 1 extra point).
    pub sentence_point: bool,
}

/// Per-speaker DSS result.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerDss {
    /// Speaker identifier.
    pub speaker: String,
    /// Number of utterances scored.
    pub utterances_scored: UtteranceCount,
    /// Individual utterance scores.
    pub scores: Vec<UtteranceScore>,
    /// Grand total (sum of all utterance totals + sentence points).
    pub grand_total: u32,
    /// DSS score (grand total / number of utterances scored).
    pub dss_score: AnalysisScore,
}

/// Typed output for the DSS command.
#[derive(Debug, Clone, Serialize)]
pub struct DssResult {
    /// Per-speaker results.
    pub speakers: Vec<SpeakerDss>,
}

impl DssResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("dss");
        for sp in &self.speakers {
            let mut section = Section::with_fields(
                format!("Speaker: {}", sp.speaker),
                indexmap::IndexMap::new(),
            );
            section.fields.insert(
                "Utterances scored".to_owned(),
                sp.utterances_scored.to_string(),
            );
            section
                .fields
                .insert("Grand total".to_owned(), sp.grand_total.to_string());
            section
                .fields
                .insert("DSS score".to_owned(), format!("{:.2}", sp.dss_score));
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for DssResult {
    /// Render DSS scores as a human-readable text summary.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render DSS scores in CLAN-compatible format.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for sp in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", sp.speaker));
            out.push_str(&format!("  Utterances scored: {}\n", sp.utterances_scored));
            out.push_str(&format!("  Grand total: {}\n", sp.grand_total));
            out.push_str(&format!("  DSS score: {:.2}\n\n", sp.dss_score));
        }
        out
    }
}

/// Accumulated state for DSS.
#[derive(Debug, Default)]
pub struct DssState {
    /// Per-speaker: list of (mor_items, main_text) pairs for scoring.
    utterances: BTreeMap<String, Vec<(Vec<Mor>, String)>>,
}

/// DSS command implementation.
///
/// Collects utterances with `%mor` tiers during processing, then scores
/// them against the loaded rule set during finalization. Up to
/// `config.max_utterances` utterances are scored per speaker.
pub struct DssCommand {
    /// Command configuration.
    config: DssConfig,
    /// Loaded DSS scoring rules.
    rules: DssRuleSet,
}

impl DssCommand {
    /// Create a new DSS command, optionally loading rules from a file.
    pub fn new(config: DssConfig) -> Result<Self, TransformError> {
        let rules = if let Some(ref path) = config.rules_path {
            load_dss_rules(path)?
        } else {
            DssRuleSet::default()
        };
        Ok(Self { config, rules })
    }
}

/// Score a single utterance's typed `%mor` items against the DSS rules.
///
/// Iterates over each `Mor` item and checks whether it matches any rule's
/// POS patterns via [`mor::mor_pattern_matches`]. Returns per-category point
/// totals and an overall total.
pub fn score_utterance(
    items: &[Mor],
    rules: &DssRuleSet,
) -> (BTreeMap<String, ScorePoints>, ScorePoints) {
    let mut category_points: BTreeMap<String, ScorePoints> = BTreeMap::new();
    let mut total: ScorePoints = 0;

    for rule in &rules.rules {
        let matched = items.iter().any(|item| {
            rule.patterns
                .iter()
                .any(|pat| mor::mor_pattern_matches(item, pat))
        });

        if matched {
            let pts = rule.points;
            *category_points.entry(rule.category.clone()).or_insert(0) += pts;
            total += pts;
        }
    }

    (category_points, total)
}

/// Check if an utterance appears to be a complete sentence.
///
/// Uses a heuristic: the typed `%mor` items must contain both a subject-like
/// POS tag and a verb-like POS tag. Complete sentences receive an additional
/// sentence point in DSS scoring.
///
/// Handles both legacy CLAN tags (`pro:sub`, `n`, `v`, `cop`) and modern UD
/// tags (`pron`, `propn`, `noun`, `verb`). The `any_item_has_pos` function
/// uses `starts_with`, so `"n"` matches both `"n"` and `"noun"`.
pub fn is_complete_sentence(items: &[Mor]) -> bool {
    let has_subject = mor::any_item_has_pos(items, "pro:sub")
        || mor::any_item_has_pos(items, "pro:per")
        || mor::any_item_has_pos(items, "pron")  // UD flat pronoun tag
        || mor::any_item_has_pos(items, "propn") // UD proper nouns
        || mor::any_item_has_pos(items, "n"); // legacy "n"/"n:prop" + UD "noun"
    let has_verb = mor::any_item_has_pos(items, "v") // legacy "v"/"v:aux" + UD "verb"
        || mor::any_item_has_pos(items, "cop")
        || mor::any_item_has_pos(items, "aux");
    has_subject && has_verb
}

impl AnalysisCommand for DssCommand {
    type Config = DssConfig;
    type State = DssState;
    type Output = DssResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();
        let main_text = spoken_main_text(&utterance.main);

        if let Some(mor_tier) = mor::extract_mor_tier(utterance) {
            let items: Vec<Mor> = mor_tier.items().to_vec();
            state
                .utterances
                .entry(speaker)
                .or_default()
                .push((items, main_text));
        }
    }

    fn finalize(&self, state: Self::State) -> DssResult {
        let mut speakers = Vec::new();

        for (speaker, utts) in state.utterances {
            let max = self.config.max_utterances.get().min(utts.len());
            let mut scores = Vec::new();
            let mut grand_total = 0u32;

            for (i, (mor_items, main_text)) in utts.iter().take(max).enumerate() {
                let (category_points, total) = score_utterance(mor_items, &self.rules);
                let sentence_point = is_complete_sentence(mor_items);
                let utt_total = total + u32::from(sentence_point);
                grand_total += utt_total;

                // Abbreviate main text for display (CLAN convention: 60 chars max)
                const MAX_DISPLAY_LEN: usize = 60;
                let display_text = if main_text.len() > MAX_DISPLAY_LEN {
                    format!("{}...", &main_text[..MAX_DISPLAY_LEN - 3])
                } else {
                    main_text.clone()
                };

                scores.push(UtteranceScore {
                    index: i + 1,
                    text: display_text,
                    category_points,
                    total: utt_total,
                    sentence_point,
                });
            }

            let dss_score = if max > 0 {
                grand_total as f64 / max as f64
            } else {
                0.0
            };

            speakers.push(SpeakerDss {
                speaker,
                utterances_scored: max as u64,
                scores,
                grand_total,
                dss_score,
            });
        }

        DssResult { speakers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use smallvec::smallvec;
    use talkbank_model::{MorFeature, MorWord};

    fn mor(pos: &str, lemma: &str, features: &[&str]) -> Mor {
        let mut word = MorWord::new(pos, lemma);
        word.features = features.iter().map(MorFeature::new).collect();
        Mor {
            main: word,
            post_clitics: smallvec![],
        }
    }

    #[test]
    fn dss_empty() {
        let cmd = DssCommand::new(DssConfig::default()).unwrap();
        let state = DssState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }

    #[test]
    fn score_utterance_basic() {
        let rules = DssRuleSet::default();
        let items = vec![
            mor("pro:sub", "I", &[]),
            mor("v", "want", &[]),
            mor("det:art", "the", &[]),
            mor("n", "ball", &[]),
        ];
        let (points, total) = score_utterance(&items, &rules);
        assert!(total > 0);
        assert!(points.contains_key("personal_pronouns"));
        assert!(points.contains_key("main_verbs"));
        assert!(points.contains_key("articles"));
    }

    #[test]
    fn score_utterance_past_tense() {
        let rules = DssRuleSet::default();
        let items = vec![mor("pro:sub", "I", &[]), mor("v", "walk", &["PAST"])];
        let (points, _total) = score_utterance(&items, &rules);
        // The "v-PAST" compound pattern now correctly matches typed POS+feature
        assert!(points.contains_key("past_tense"));
    }

    #[test]
    fn is_complete_sentence_check() {
        let complete = vec![
            mor("pro:sub", "I", &[]),
            mor("v", "want", &[]),
            mor("det:art", "the", &[]),
            mor("n", "ball", &[]),
        ];
        assert!(is_complete_sentence(&complete));

        let no_verb = vec![mor("det:art", "the", &[]), mor("n", "ball", &[])];
        assert!(!is_complete_sentence(&no_verb));

        let no_subject = vec![mor("v", "run", &[])];
        assert!(!is_complete_sentence(&no_subject));
    }

    #[test]
    fn is_complete_sentence_with_copula() {
        let items = vec![
            mor("n", "dog", &[]),
            mor("cop", "be", &["3S"]),
            mor("adj", "big", &[]),
        ];
        assert!(is_complete_sentence(&items));
    }

    #[test]
    fn is_complete_sentence_with_proper_noun() {
        let items = vec![mor("n:prop", "John", &[]), mor("v", "run", &["PAST"])];
        assert!(is_complete_sentence(&items));
    }

    // --- UD format tests ---

    #[test]
    fn is_complete_sentence_ud_tags() {
        // UD: pron + verb
        let items = vec![
            mor("pron", "I", &["Prs", "Nom", "S1"]),
            mor("verb", "want", &["Fin", "Ind", "Pres"]),
            mor("noun", "cookie", &["Plur"]),
        ];
        assert!(is_complete_sentence(&items));
    }

    #[test]
    fn is_complete_sentence_ud_propn_subject() {
        // UD: propn as subject + verb
        let items = vec![
            mor("propn", "Mommy", &[]),
            mor("aux", "will", &[]),
            mor("verb", "get", &["Inf"]),
        ];
        assert!(is_complete_sentence(&items));
    }

    #[test]
    fn is_complete_sentence_ud_noun_subject() {
        // UD: noun as subject + aux (copula in UD is "aux")
        let items = vec![
            mor("noun", "dog", &[]),
            mor("aux", "be", &["Fin", "Ind", "Pres", "S3"]),
            mor("adj", "big", &[]),
        ];
        assert!(is_complete_sentence(&items));
    }

    #[test]
    fn score_utterance_ud_format() {
        let rules = DssRuleSet::default();
        // UD format utterance: "I want the ball"
        let items = vec![
            mor("pron", "I", &["Prs", "Nom", "S1"]),
            mor("verb", "want", &["Fin", "Ind", "Pres"]),
            mor("det", "the", &[]),
            mor("noun", "ball", &[]),
        ];
        let (_points, total) = score_utterance(&items, &rules);
        // "verb" matches "v" rule (starts_with "v"), so main_verbs should score
        assert!(total > 0);
    }
}
