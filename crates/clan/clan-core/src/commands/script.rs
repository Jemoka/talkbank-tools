//! SCRIPT — Compare utterances to a template script.
//!
//! Compares subject CHAT data against an ideal template file to compute
//! accuracy metrics: words produced vs. expected, correct matches,
//! omissions (in template but not produced), and additions (produced but
//! not in template). Useful for evaluating scripted language samples
//! such as picture descriptions or story retells.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409234)
//! for the original SCRIPT command specification.
//!
//! # Differences from CLAN
//!
//! - Template file is parsed into a typed AST (not raw text comparison).
//! - Word matching uses `NormalizedWord` for case-insensitive comparison.
//! - Omissions and additions are computed from frequency maps rather than
//!   positional alignment, which may produce different results when word
//!   order matters.
//! - Output supports text, JSON, and CSV formats.
//!
//! # Algorithm
//!
//! 1. Parse the template CHAT file and build a word frequency map (ideal
//!    counts).
//! 2. For each subject utterance, accumulate word frequency counts.
//! 3. At finalization, compute per-word matches (minimum of ideal and
//!    actual), omissions, and additions.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use talkbank_model::ParseValidateOptions;
use talkbank_model::Utterance;

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section,
    TransformError, countable_words,
};

/// Configuration for the SCRIPT command.
#[derive(Debug, Clone)]
pub struct ScriptConfig {
    /// Path to template/script file.
    pub template_path: PathBuf,
}

/// Per-file accuracy metrics.
#[derive(Debug, Clone, Serialize)]
pub struct FileMetrics {
    /// Filename.
    pub filename: String,
    /// Words produced by subject.
    pub words_produced: u64,
    /// Words expected from template.
    pub words_ideal: u64,
    /// Correct words (matched).
    pub words_correct: u64,
    /// Omitted words (in template but not produced).
    pub words_omitted: u64,
    /// Added words (produced but not in template).
    pub words_added: u64,
    /// Percentage correct.
    pub pct_correct: f64,
}

/// Typed output for the SCRIPT command.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptResult {
    /// Per-file metrics.
    pub files: Vec<FileMetrics>,
    /// Overall metrics.
    /// Total words produced across all files.
    pub total_produced: u64,
    /// Total words expected from template.
    pub total_ideal: u64,
    /// Total correct words.
    pub total_correct: u64,
    /// Total omitted words.
    pub total_omitted: u64,
    /// Total added words.
    pub total_added: u64,
    /// Overall percentage correct.
    pub overall_pct: f64,
}

impl ScriptResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("script");
        for file in &self.files {
            let mut section = Section::with_fields(
                format!("File: {}", file.filename),
                indexmap::IndexMap::new(),
            );
            section
                .fields
                .insert("Words produced".to_owned(), file.words_produced.to_string());
            section
                .fields
                .insert("Words ideal".to_owned(), file.words_ideal.to_string());
            section
                .fields
                .insert("Words correct".to_owned(), file.words_correct.to_string());
            section
                .fields
                .insert("Words omitted".to_owned(), file.words_omitted.to_string());
            section
                .fields
                .insert("Words added".to_owned(), file.words_added.to_string());
            section
                .fields
                .insert("% correct".to_owned(), format!("{:.1}%", file.pct_correct));
            result.add_section(section);
        }

        let mut summary = Section::with_fields("Summary".to_owned(), indexmap::IndexMap::new());
        summary
            .fields
            .insert("Total produced".to_owned(), self.total_produced.to_string());
        summary
            .fields
            .insert("Total ideal".to_owned(), self.total_ideal.to_string());
        summary
            .fields
            .insert("Total correct".to_owned(), self.total_correct.to_string());
        summary
            .fields
            .insert("Overall %".to_owned(), format!("{:.1}%", self.overall_pct));
        result.add_section(summary);
        result
    }
}

impl CommandOutput for ScriptResult {
    /// Render per-file accuracy metrics and overall summary.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible per-file accuracy report.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for file in &self.files {
            out.push_str(&format!("File: {}\n", file.filename));
            out.push_str(&format!("  Words produced: {}\n", file.words_produced));
            out.push_str(&format!("  Words ideal:    {}\n", file.words_ideal));
            out.push_str(&format!("  Words correct:  {}\n", file.words_correct));
            out.push_str(&format!("  Words omitted:  {}\n", file.words_omitted));
            out.push_str(&format!("  Words added:    {}\n", file.words_added));
            out.push_str(&format!("  % correct:      {:.1}%\n\n", file.pct_correct));
        }
        out.push_str(&format!(
            "Overall: {:.1}% correct ({} of {} ideal words)\n",
            self.overall_pct, self.total_correct, self.total_ideal
        ));
        out
    }
}

/// Load ideal word counts from a template CHAT file.
///
/// Parses the template, extracts all words from main tiers (lowercased),
/// and returns a frequency map. Punctuation terminators are excluded.
fn load_template(path: &std::path::Path) -> Result<BTreeMap<String, u64>, TransformError> {
    let content_str = std::fs::read_to_string(path).map_err(TransformError::Io)?;
    let chat =
        talkbank_transform::parse_and_validate(&content_str, ParseValidateOptions::default())
            .map_err(|e| TransformError::Parse(format!("Template: {e}")))?;

    let mut word_counts: BTreeMap<String, u64> = BTreeMap::new();
    for utt in chat.utterances() {
        for word in countable_words(&utt.main.content.content) {
            *word_counts
                .entry(word.cleaned_text().to_lowercase())
                .or_insert(0u64) += 1;
        }
    }

    Ok(word_counts)
}

/// Accumulated state for SCRIPT.
#[derive(Debug, Default)]
pub struct ScriptState {
    /// Per-word counts in subject data.
    word_counts: BTreeMap<String, u64>,
    /// Total words produced.
    total_produced: u64,
}

/// SCRIPT command implementation.
///
/// Holds the parsed template word counts and compares subject utterances
/// against them at finalization.
pub struct ScriptCommand {
    _config: ScriptConfig,
    /// Ideal word counts from template.
    template: BTreeMap<String, u64>,
}

impl ScriptCommand {
    /// Create a new SCRIPT command, loading the template file.
    pub fn new(config: ScriptConfig) -> Result<Self, TransformError> {
        let template = load_template(&config.template_path)?;
        Ok(Self {
            _config: config,
            template,
        })
    }
}

impl AnalysisCommand for ScriptCommand {
    type Config = ScriptConfig;
    type State = ScriptState;
    type Output = ScriptResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        for word in countable_words(&utterance.main.content.content) {
            *state
                .word_counts
                .entry(word.cleaned_text().to_lowercase())
                .or_insert(0) += 1;
            state.total_produced += 1;
        }
    }

    fn finalize(&self, state: Self::State) -> ScriptResult {
        let mut correct = 0u64;
        let mut ideal_total = 0u64;

        for (word, ideal_count) in &self.template {
            ideal_total += ideal_count;
            let actual = state.word_counts.get(word).copied().unwrap_or(0);
            correct += actual.min(*ideal_count);
        }

        let omitted = ideal_total.saturating_sub(correct);
        let added = state.total_produced.saturating_sub(correct);
        let pct = if ideal_total > 0 {
            correct as f64 / ideal_total as f64 * 100.0
        } else {
            0.0
        };

        let file_metrics = FileMetrics {
            filename: "aggregated".to_owned(),
            words_produced: state.total_produced,
            words_ideal: ideal_total,
            words_correct: correct,
            words_omitted: omitted,
            words_added: added,
            pct_correct: pct,
        };

        ScriptResult {
            files: vec![file_metrics],
            total_produced: state.total_produced,
            total_ideal: ideal_total,
            total_correct: correct,
            total_omitted: omitted,
            total_added: added,
            overall_pct: pct,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_empty() {
        // Without a template file, test that finalize works with empty state
        let template = BTreeMap::new();
        let cmd = ScriptCommand {
            _config: ScriptConfig {
                template_path: PathBuf::from("nonexistent"),
            },
            template,
        };
        let state = ScriptState::default();
        let result = cmd.finalize(state);
        assert_eq!(result.total_produced, 0);
        assert_eq!(result.total_ideal, 0);
    }

    #[test]
    fn script_perfect_match() {
        let mut template = BTreeMap::new();
        template.insert("hello".to_owned(), 2);
        template.insert("world".to_owned(), 1);

        let cmd = ScriptCommand {
            _config: ScriptConfig {
                template_path: PathBuf::from("test"),
            },
            template,
        };

        let mut state = ScriptState::default();
        state.word_counts.insert("hello".to_owned(), 2);
        state.word_counts.insert("world".to_owned(), 1);
        state.total_produced = 3;

        let result = cmd.finalize(state);
        assert_eq!(result.total_correct, 3);
        assert_eq!(result.total_omitted, 0);
        assert_eq!(result.total_added, 0);
        assert!((result.overall_pct - 100.0).abs() < f64::EPSILON);
    }
}
