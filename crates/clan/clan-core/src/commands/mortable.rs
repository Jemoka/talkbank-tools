//! MORTABLE — Cross-tabulation of morphological categories.
//!
//! Produces a per-speaker frequency table of morphosyntactic categories by
//! matching POS tags from the `%mor` tier against patterns defined in a
//! language-specific script file.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409286)
//! for the original MORTABLE command specification.
//!
//! # Differences from CLAN
//!
//! - POS matching operates on parsed `%mor` tier data rather than raw text
//!   line scanning.
//! - Script file format is compatible with CLAN's `.cut` files.
//! - Output supports text, JSON, and CSV formats.
//! - `BTreeMap` ordering ensures deterministic output across runs.
//!
//! # External Data
//!
//! Requires a language script file (e.g., `eng.cut`) that defines patterns
//! and their labels for categorizing morphemes from the `%mor` tier. Each
//! rule line contains a quoted label and `+`/`-` prefixed POS patterns.
//! Rules can be grouped as OR (first match wins) or AND (all must match).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use talkbank_model::{DependentTier, Utterance};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    WordCount, mor_item_pos_tags,
};

/// Configuration for the MORTABLE command.
#[derive(Debug, Clone)]
pub struct MortableConfig {
    /// Path to the language script file.
    pub script_path: PathBuf,
}

/// A rule pattern from the script file.
#[derive(Debug, Clone)]
pub struct MortableRule {
    /// Label for this pattern category.
    pub label: String,
    /// POS patterns to match (prefixed with + for include, - for exclude).
    pub patterns: Vec<String>,
    /// Whether this is an OR group (first match wins) or AND group (all match).
    pub is_or: bool,
}

/// Per-speaker morphological category counts.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerMortable {
    /// Speaker identifier.
    pub speaker: String,
    /// Category label → count.
    pub categories: BTreeMap<String, u64>,
    /// Total words counted.
    pub total_words: WordCount,
}

/// Typed output for the MORTABLE command.
#[derive(Debug, Clone, Serialize)]
pub struct MortableResult {
    /// Per-speaker category frequencies.
    pub speakers: Vec<SpeakerMortable>,
    /// Ordered list of category labels.
    pub labels: Vec<String>,
}

impl MortableResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("mortable");
        for speaker in &self.speakers {
            let mut headers = vec!["Total".to_owned()];
            headers.extend(self.labels.iter().cloned());

            let mut values = vec![speaker.total_words.to_string()];
            for label in &self.labels {
                let count = speaker.categories.get(label).copied().unwrap_or(0);
                values.push(count.to_string());
            }

            let section = Section::with_table(
                format!("Speaker: {}", speaker.speaker),
                headers,
                vec![TableRow { values }],
            );
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for MortableResult {
    /// Render per-speaker category frequency table.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible tab-separated frequency table.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        // Header row
        out.push_str("Speaker\tTotal");
        for label in &self.labels {
            out.push_str(&format!("\t{label}"));
        }
        out.push('\n');

        for speaker in &self.speakers {
            out.push_str(&format!("{}\t{}", speaker.speaker, speaker.total_words));
            for label in &self.labels {
                let count = speaker.categories.get(label).copied().unwrap_or(0);
                out.push_str(&format!("\t{count}"));
            }
            out.push('\n');
        }
        out
    }
}

/// Accumulated state for MORTABLE across all files.
#[derive(Debug, Default)]
pub struct MortableState {
    /// Speaker → (category → count).
    speakers: BTreeMap<String, BTreeMap<String, u64>>,
    /// Speaker → total word count.
    totals: BTreeMap<String, u64>,
}

/// MORTABLE command implementation.
///
/// For each utterance's `%mor` tier, extracts POS tags and classifies
/// them against the loaded rule set. Rules can use OR mode (first
/// matching rule wins) or AND mode (all patterns must match).
pub struct MortableCommand {
    rules: Vec<MortableRule>,
    labels: Vec<String>,
}

impl MortableCommand {
    /// Create a new MORTABLE command, loading rules from script file.
    pub fn new(config: MortableConfig) -> Result<Self, crate::framework::TransformError> {
        let (rules, labels) = load_script(&config.script_path)?;
        Ok(Self { rules, labels })
    }
}

/// Check if a POS tag matches a pattern (case-insensitive).
///
/// Pattern prefixes:
/// - `+tag` -- positive match: POS equals `tag` or starts with `tag:`
/// - `-tag` -- negative match: POS does NOT equal `tag` or start with `tag:`
/// - bare `tag` -- exact match (case-insensitive)
fn pos_matches(pos: &str, pattern: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let pos_lower = pos.to_lowercase();

    if let Some(pat) = pattern_lower.strip_prefix('+') {
        pos_lower == pat || pos_lower.starts_with(&format!("{pat}:"))
    } else if let Some(pat) = pattern_lower.strip_prefix('-') {
        !(pos_lower == pat || pos_lower.starts_with(&format!("{pat}:")))
    } else {
        pos_lower == pattern_lower
    }
}

impl AnalysisCommand for MortableCommand {
    type Config = MortableConfig;
    type State = MortableState;
    type Output = MortableResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();

        for dep in &utterance.dependent_tiers {
            if let DependentTier::Mor(tier) = dep {
                for pos in mor_item_pos_tags(tier) {
                    *state.totals.entry(speaker.clone()).or_insert(0) += 1;

                    // Match against rules
                    let speaker_cats = state.speakers.entry(speaker.clone()).or_default();
                    for rule in &self.rules {
                        let matched = if rule.patterns.iter().any(|p| p.starts_with('+')) {
                            rule.patterns.iter().any(|p| pos_matches(&pos, p))
                        } else {
                            rule.patterns.iter().all(|p| pos_matches(&pos, p))
                        };

                        if matched {
                            *speaker_cats.entry(rule.label.clone()).or_insert(0) += 1;
                            if rule.is_or {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    fn finalize(&self, state: Self::State) -> MortableResult {
        let speakers: Vec<SpeakerMortable> = state
            .speakers
            .into_iter()
            .map(|(speaker, categories)| {
                let total = state.totals.get(&speaker).copied().unwrap_or(0);
                SpeakerMortable {
                    speaker,
                    categories,
                    total_words: total,
                }
            })
            .collect();

        MortableResult {
            speakers,
            labels: self.labels.clone(),
        }
    }
}

/// Load a MORTABLE script file defining category rules.
///
/// The file format uses OR/AND mode keywords, quoted labels, and `+`/`-`
/// prefixed POS patterns. Lines starting with `#` or `;` are comments.
///
/// # Format
///
/// ```text
/// OR
/// "Nouns" +n
/// "Verbs" +v +cop +aux
/// AND
/// "Past Tense Verbs" +v +PAST
/// ```
fn load_script(
    path: &Path,
) -> Result<(Vec<MortableRule>, Vec<String>), crate::framework::TransformError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        crate::framework::TransformError::Transform(format!(
            "Cannot read script file '{}': {e}",
            path.display()
        ))
    })?;

    let mut rules = Vec::new();
    let mut labels = Vec::new();
    let mut current_is_or = true;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.eq_ignore_ascii_case("OR") {
            current_is_or = true;
            continue;
        }
        if line.eq_ignore_ascii_case("AND") {
            current_is_or = false;
            continue;
        }

        // Extract quoted label and patterns
        if let Some(label_start) = line.find('"')
            && let Some(label_end) = line[label_start + 1..].find('"')
        {
            let label = line[label_start + 1..label_start + 1 + label_end].to_owned();
            let rest = &line[label_start + 1 + label_end + 1..];
            let patterns: Vec<String> = rest
                .split_whitespace()
                .filter(|s| s.starts_with('+') || s.starts_with('-'))
                .map(|s| s.to_owned())
                .collect();

            if !patterns.is_empty() {
                labels.push(label.clone());
                rules.push(MortableRule {
                    label,
                    patterns,
                    is_or: current_is_or,
                });
            }
        }
    }

    Ok((rules, labels))
}
