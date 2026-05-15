//! IPSYN — Index of Productive Syntax.
//!
//! Computes a syntactic complexity score by awarding points for distinct
//! syntactic structures observed in a child's utterances. Each structure
//! type (rule) can earn at most 2 points -- one per distinct utterance in
//! which the structure appears. The total across all rules yields the
//! IPSyn score.
//!
//! Rules are organized into four categories: Noun Phrase (N), Verb
//! Phrase (V), Question (Q), and Sentence Structure (S). The default
//! rule set provides a representative subset of the full ~56 English
//! IPSyn rules; a custom rule file can be supplied for the complete set
//! or for other languages.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409276)
//! for the original IPSYN command specification.
//!
//! # Differences from CLAN
//!
//! - The built-in rule set is a simplified subset. For full 56-rule coverage,
//!   supply the official IPSYN rules file via `rules_path`.
//! - Pattern matching uses typed `%mor` AST (POS tags and features) rather
//!   than substring matching on serialized text.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;
use talkbank_model::{Mor, Utterance};

use crate::framework::mor;
use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, ScorePoints,
    Section, TableRow, TransformError, UtteranceCount, UtteranceLimit,
};

/// Configuration for the IPSYN command.
#[derive(Debug, Clone)]
pub struct IpsynConfig {
    /// Path to IPSYN rules file.
    pub rules_path: Option<PathBuf>,
    /// Maximum number of utterances to analyze (default: 100).
    pub max_utterances: UtteranceLimit,
}

impl Default for IpsynConfig {
    fn default() -> Self {
        Self {
            rules_path: None,
            max_utterances: UtteranceLimit::new(100),
        }
    }
}

/// An IPSYN rule: pattern to match on %mor/%gra tiers.
#[derive(Debug, Clone)]
pub struct IpsynRule {
    /// Rule name (e.g., "S1", "S2", "V1", "Q1").
    pub name: String,
    /// Category (S=sentence, V=verb, Q=question, N=noun phrase).
    pub category: char,
    /// POS patterns that must be present in the utterance.
    pub include_patterns: Vec<String>,
    /// POS patterns that must NOT be present (exclusions).
    pub exclude_patterns: Vec<String>,
    /// Description of the syntactic structure.
    pub description: String,
}

/// A loaded IPSYN rule set.
#[derive(Debug, Clone)]
pub struct IpsynRuleSet {
    /// All rules.
    pub rules: Vec<IpsynRule>,
}

impl Default for IpsynRuleSet {
    fn default() -> Self {
        Self {
            rules: default_english_ipsyn_rules(),
        }
    }
}

/// Default English IPSYN rules (simplified core set).
///
/// Full IPSYN has ~56 rules. This provides a representative subset.
fn default_english_ipsyn_rules() -> Vec<IpsynRule> {
    vec![
        // Noun Phrase rules
        IpsynRule {
            name: "N1".to_owned(),
            category: 'N',
            include_patterns: vec!["n|".to_owned()],
            exclude_patterns: vec![],
            description: "Noun".to_owned(),
        },
        IpsynRule {
            name: "N2".to_owned(),
            category: 'N',
            include_patterns: vec!["det|".to_owned(), "n|".to_owned()],
            exclude_patterns: vec![],
            description: "Determiner + Noun".to_owned(),
        },
        IpsynRule {
            name: "N3".to_owned(),
            category: 'N',
            include_patterns: vec!["adj|".to_owned(), "n|".to_owned()],
            exclude_patterns: vec![],
            description: "Adjective + Noun".to_owned(),
        },
        IpsynRule {
            name: "N4".to_owned(),
            category: 'N',
            include_patterns: vec!["n|".to_owned(), "POSS".to_owned()],
            exclude_patterns: vec![],
            description: "Possessive noun".to_owned(),
        },
        IpsynRule {
            name: "N5".to_owned(),
            category: 'N',
            include_patterns: vec!["pro:sub|".to_owned()],
            exclude_patterns: vec![],
            description: "Subject pronoun".to_owned(),
        },
        IpsynRule {
            name: "N6".to_owned(),
            category: 'N',
            include_patterns: vec!["pro:obj|".to_owned()],
            exclude_patterns: vec![],
            description: "Object pronoun".to_owned(),
        },
        // Verb Phrase rules
        IpsynRule {
            name: "V1".to_owned(),
            category: 'V',
            include_patterns: vec!["v|".to_owned()],
            exclude_patterns: vec![],
            description: "Verb".to_owned(),
        },
        IpsynRule {
            name: "V2".to_owned(),
            category: 'V',
            include_patterns: vec!["v|".to_owned(), "PAST".to_owned()],
            exclude_patterns: vec![],
            description: "Past tense verb".to_owned(),
        },
        IpsynRule {
            name: "V3".to_owned(),
            category: 'V',
            include_patterns: vec!["cop|".to_owned()],
            exclude_patterns: vec![],
            description: "Copula".to_owned(),
        },
        IpsynRule {
            name: "V4".to_owned(),
            category: 'V',
            include_patterns: vec!["aux|".to_owned()],
            exclude_patterns: vec![],
            description: "Auxiliary verb".to_owned(),
        },
        IpsynRule {
            name: "V5".to_owned(),
            category: 'V',
            include_patterns: vec!["mod|".to_owned()],
            exclude_patterns: vec![],
            description: "Modal verb".to_owned(),
        },
        // Question rules
        IpsynRule {
            name: "Q1".to_owned(),
            category: 'Q',
            include_patterns: vec!["pro:wh|".to_owned()],
            exclude_patterns: vec![],
            description: "Wh-word".to_owned(),
        },
        IpsynRule {
            name: "Q2".to_owned(),
            category: 'Q',
            include_patterns: vec!["adv:wh|".to_owned()],
            exclude_patterns: vec![],
            description: "Wh-adverb".to_owned(),
        },
        // Sentence Structure rules
        IpsynRule {
            name: "S1".to_owned(),
            category: 'S',
            include_patterns: vec!["pro:sub|".to_owned(), "v|".to_owned()],
            exclude_patterns: vec![],
            description: "Subject-Verb".to_owned(),
        },
        IpsynRule {
            name: "S2".to_owned(),
            category: 'S',
            include_patterns: vec!["neg|".to_owned()],
            exclude_patterns: vec![],
            description: "Negation".to_owned(),
        },
        IpsynRule {
            name: "S3".to_owned(),
            category: 'S',
            include_patterns: vec!["conj:coo|".to_owned()],
            exclude_patterns: vec![],
            description: "Coordinating conjunction".to_owned(),
        },
        IpsynRule {
            name: "S4".to_owned(),
            category: 'S',
            include_patterns: vec!["conj:sub|".to_owned()],
            exclude_patterns: vec![],
            description: "Subordinating conjunction".to_owned(),
        },
        IpsynRule {
            name: "S5".to_owned(),
            category: 'S',
            include_patterns: vec!["prep|".to_owned()],
            exclude_patterns: vec![],
            description: "Prepositional phrase".to_owned(),
        },
    ]
}

/// Load IPSYN rules from a tab-delimited file.
///
/// Each non-comment line has the format:
/// `NAME\tinclude_pat1,include_pat2\t[exclude_pat1,...]\tdescription`
///
/// Lines starting with `#` and blank lines are skipped.
pub fn load_ipsyn_rules(path: &std::path::Path) -> Result<IpsynRuleSet, TransformError> {
    let content = std::fs::read_to_string(path).map_err(TransformError::Io)?;
    let mut rules = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Format: NAME CATEGORY include_pat1,include_pat2 [exclude_pat1,exclude_pat2] description
        let parts: Vec<&str> = line.splitn(4, '\t').collect();
        if parts.len() >= 3 {
            let name = parts[0].to_owned();
            let category = parts[0].chars().next().unwrap_or('S');
            let include_patterns: Vec<String> =
                parts[1].split(',').map(|s| s.trim().to_owned()).collect();
            let exclude_patterns: Vec<String> = if parts.len() > 3 && !parts[2].is_empty() {
                parts[2].split(',').map(|s| s.trim().to_owned()).collect()
            } else {
                vec![]
            };
            let description = if parts.len() > 3 {
                parts[3].to_owned()
            } else {
                parts.last().unwrap_or(&"").to_string()
            };

            rules.push(IpsynRule {
                name,
                category,
                include_patterns,
                exclude_patterns,
                description,
            });
        }
    }

    Ok(IpsynRuleSet { rules })
}

/// Per-rule match result.
#[derive(Debug, Clone, Serialize)]
pub struct RuleMatch {
    /// Rule name.
    pub rule: String,
    /// Category.
    pub category: String,
    /// Number of distinct utterances matching (max 2 → 1 point each).
    pub matches: u32,
    /// Points awarded (min(matches, 2)).
    pub points: ScorePoints,
}

/// Per-speaker IPSYN result.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerIpsyn {
    /// Speaker identifier.
    pub speaker: String,
    /// Number of utterances analyzed.
    pub utterances_analyzed: UtteranceCount,
    /// Per-rule match results.
    pub rule_matches: Vec<RuleMatch>,
    /// Total IPSYN score.
    pub total_score: ScorePoints,
    /// Scores by category.
    pub category_scores: BTreeMap<String, ScorePoints>,
}

/// Typed output for the IPSYN command.
#[derive(Debug, Clone, Serialize)]
pub struct IpsynResult {
    /// Per-speaker results.
    pub speakers: Vec<SpeakerIpsyn>,
}

impl IpsynResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("ipsyn");
        for sp in &self.speakers {
            let rows: Vec<TableRow> = sp
                .rule_matches
                .iter()
                .filter(|r| r.points > 0)
                .map(|r| TableRow {
                    values: vec![
                        r.rule.clone(),
                        r.category.clone(),
                        r.matches.to_string(),
                        r.points.to_string(),
                    ],
                })
                .collect();
            let mut section = Section::with_table(
                format!("Speaker: {}", sp.speaker),
                vec![
                    "Rule".to_owned(),
                    "Category".to_owned(),
                    "Matches".to_owned(),
                    "Points".to_owned(),
                ],
                rows,
            );
            section.fields.insert(
                "Utterances analyzed".to_owned(),
                sp.utterances_analyzed.to_string(),
            );
            section
                .fields
                .insert("Total IPSYN".to_owned(), sp.total_score.to_string());
            for (cat, score) in &sp.category_scores {
                section
                    .fields
                    .insert(format!("{cat} score"), score.to_string());
            }
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for IpsynResult {
    /// Render per-speaker rule-match tables and scores.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible per-speaker summary.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for sp in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", sp.speaker));
            out.push_str(&format!(
                "  Utterances: {}\n  Total IPSYN: {}\n",
                sp.utterances_analyzed, sp.total_score
            ));
            for (cat, score) in &sp.category_scores {
                out.push_str(&format!("  {cat}: {score}\n"));
            }
            out.push('\n');
        }
        out
    }
}

/// Accumulated state for IPSYN.
#[derive(Debug, Default)]
pub struct IpsynState {
    /// Per-speaker: list of typed %mor items per utterance.
    utterances: BTreeMap<String, Vec<Vec<Mor>>>,
}

/// IPSYN command implementation.
///
/// Scans `%mor` tiers for each utterance, collecting per-speaker typed
/// morphological items. At finalization, each rule is tested against the
/// first `max_utterances` utterances; a rule scores 1 point per distinct
/// matching utterance (capped at 2).
pub struct IpsynCommand {
    config: IpsynConfig,
    rules: IpsynRuleSet,
}

impl IpsynCommand {
    /// Create a new IPSYN command.
    pub fn new(config: IpsynConfig) -> Result<Self, TransformError> {
        let rules = if let Some(ref path) = config.rules_path {
            load_ipsyn_rules(path)?
        } else {
            IpsynRuleSet::default()
        };
        Ok(Self { config, rules })
    }
}

/// Check if a set of typed `%mor` items matches an IPSYN rule.
///
/// All `include_patterns` must match at least one item (via
/// [`mor::mor_pattern_matches`]), and none of the `exclude_patterns` may
/// match any item.
///
/// # Examples
///
/// ```
/// # use talkbank_model::{Mor, MorWord};
/// # use smallvec::smallvec;
/// # use clan_core::commands::ipsyn::{IpsynRule, rule_matches};
/// let rule = IpsynRule {
///     name: "S1".to_owned(),
///     category: 'S',
///     include_patterns: vec!["pro:sub|".to_owned(), "v|".to_owned()],
///     exclude_patterns: vec![],
///     description: "Subject-Verb".to_owned(),
/// };
/// let items = vec![
///     Mor { main: MorWord::new("pro:sub", "I"), post_clitics: smallvec![] },
///     Mor { main: MorWord::new("v", "want"), post_clitics: smallvec![] },
///     Mor { main: MorWord::new("det:art", "a"), post_clitics: smallvec![] },
///     Mor { main: MorWord::new("n", "ball"), post_clitics: smallvec![] },
/// ];
/// assert!(rule_matches(&items, &rule));
/// let no_verb = vec![
///     Mor { main: MorWord::new("det:art", "the"), post_clitics: smallvec![] },
///     Mor { main: MorWord::new("n", "ball"), post_clitics: smallvec![] },
/// ];
/// assert!(!rule_matches(&no_verb, &rule));
/// ```
pub fn rule_matches(items: &[Mor], rule: &IpsynRule) -> bool {
    // All include patterns must match at least one item
    for pattern in &rule.include_patterns {
        if !items
            .iter()
            .any(|item| mor::mor_pattern_matches(item, pattern))
        {
            return false;
        }
    }

    // No exclude patterns should match any item
    for pattern in &rule.exclude_patterns {
        if items
            .iter()
            .any(|item| mor::mor_pattern_matches(item, pattern))
        {
            return false;
        }
    }

    true
}

impl AnalysisCommand for IpsynCommand {
    type Config = IpsynConfig;
    type State = IpsynState;
    type Output = IpsynResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();

        if let Some(mor_tier) = mor::extract_mor_tier(utterance) {
            let items: Vec<Mor> = mor_tier.items().to_vec();
            state.utterances.entry(speaker).or_default().push(items);
        }
    }

    fn finalize(&self, state: Self::State) -> IpsynResult {
        let mut speakers = Vec::new();

        for (speaker, utts) in state.utterances {
            let max = self.config.max_utterances.get().min(utts.len());
            let analyze_utts = &utts[..max];

            let mut rule_matches_list = Vec::new();
            let mut total_score: ScorePoints = 0;
            let mut category_scores: BTreeMap<String, ScorePoints> = BTreeMap::new();

            for rule in &self.rules.rules {
                let mut match_utts: BTreeSet<usize> = BTreeSet::new();
                for (i, items) in analyze_utts.iter().enumerate() {
                    if rule_matches(items, rule) {
                        match_utts.insert(i);
                        if match_utts.len() >= 2 {
                            break;
                        }
                    }
                }

                let matches = match_utts.len() as u32;
                let points = matches.min(2);
                total_score += points;
                *category_scores
                    .entry(rule.category.to_string())
                    .or_insert(0) += points;

                rule_matches_list.push(RuleMatch {
                    rule: rule.name.clone(),
                    category: rule.category.to_string(),
                    matches,
                    points,
                });
            }

            speakers.push(SpeakerIpsyn {
                speaker,
                utterances_analyzed: max as u64,
                rule_matches: rule_matches_list,
                total_score,
                category_scores,
            });
        }

        IpsynResult { speakers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use smallvec::smallvec;
    use talkbank_model::{MorFeature, MorWord};

    fn mor_item(pos: &str, lemma: &str, features: &[&str]) -> Mor {
        let mut word = MorWord::new(pos, lemma);
        word.features = features.iter().map(MorFeature::new).collect();
        Mor {
            main: word,
            post_clitics: smallvec![],
        }
    }

    #[test]
    fn ipsyn_empty() {
        let cmd = IpsynCommand::new(IpsynConfig::default()).unwrap();
        let state = IpsynState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }

    #[test]
    fn rule_match_basic() {
        let rule = IpsynRule {
            name: "S1".to_owned(),
            category: 'S',
            include_patterns: vec!["pro:sub|".to_owned(), "v|".to_owned()],
            exclude_patterns: vec![],
            description: "Subject-Verb".to_owned(),
        };
        let items = vec![
            mor_item("pro:sub", "I", &[]),
            mor_item("v", "want", &[]),
            mor_item("det:art", "a", &[]),
            mor_item("n", "ball", &[]),
        ];
        assert!(rule_matches(&items, &rule));

        let no_verb = vec![mor_item("det:art", "the", &[]), mor_item("n", "ball", &[])];
        assert!(!rule_matches(&no_verb, &rule));
    }

    #[test]
    fn rule_exclude_works() {
        let rule = IpsynRule {
            name: "Test".to_owned(),
            category: 'T',
            include_patterns: vec!["v|".to_owned()],
            exclude_patterns: vec!["neg|".to_owned()],
            description: "Verb without negation".to_owned(),
        };
        let items = vec![mor_item("pro:sub", "I", &[]), mor_item("v", "want", &[])];
        assert!(rule_matches(&items, &rule));

        let with_neg = vec![
            mor_item("pro:sub", "I", &[]),
            mor_item("neg", "not", &[]),
            mor_item("v", "want", &[]),
        ];
        assert!(!rule_matches(&with_neg, &rule));
    }

    #[test]
    fn rule_match_feature_pattern() {
        let rule = IpsynRule {
            name: "N4".to_owned(),
            category: 'N',
            include_patterns: vec!["n|".to_owned(), "POSS".to_owned()],
            exclude_patterns: vec![],
            description: "Possessive noun".to_owned(),
        };
        let items = vec![mor_item("n", "dog", &["POSS"])];
        assert!(rule_matches(&items, &rule));

        let no_poss = vec![mor_item("n", "dog", &[])];
        assert!(!rule_matches(&no_poss, &rule));
    }

    #[test]
    fn rule_match_past_tense() {
        let rule = IpsynRule {
            name: "V2".to_owned(),
            category: 'V',
            include_patterns: vec!["v|".to_owned(), "PAST".to_owned()],
            exclude_patterns: vec![],
            description: "Past tense verb".to_owned(),
        };
        let items = vec![mor_item("v", "walk", &["PAST"])];
        assert!(rule_matches(&items, &rule));

        let present = vec![mor_item("v", "walk", &["3S"])];
        assert!(!rule_matches(&present, &rule));
    }
}
