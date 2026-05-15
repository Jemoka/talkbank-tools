//! CODES — Frequency table of codes from the `%cod` dependent tier.
//!
//! Reimplements CLAN's CODES command, which tabulates the frequency and
//! distribution of coding annotations found on `%cod:` dependent tiers,
//! organized by speaker. This is useful for analyzing hand-coded behavioral
//! or discourse annotations attached to transcripts.
//!
//! Codes on `%cod:` tiers typically use colon-separated hierarchical structure
//! (e.g., `AC:DI:PP`), but this implementation treats each whitespace-delimited
//! token as a single code string without parsing the internal hierarchy.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409098)
//! for the original CODES command specification.
//!
//! # Differences from CLAN
//!
//! - Codes are extracted from parsed `%cod:` dependent tier content rather
//!   than raw text line scanning.
//! - Each whitespace-delimited token is treated as a single code string;
//!   colon-separated hierarchy is preserved but not parsed into sublevels.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - `BTreeMap` ordering ensures deterministic output across runs.
//!
//! # Output
//!
//! Per-speaker frequency tables listing each code and its count, plus a
//! per-speaker total and a grand total across all speakers.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::{DependentTier, Utterance};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CodeDepth, CommandOutput, FileContext, OutputFormat, Section,
    TableRow, cod_item_values,
};

/// Configuration for the CODES command.
#[derive(Debug, Clone)]
pub struct CodesConfig {
    /// Maximum depth of code parsing (0 = all levels).
    pub max_depth: CodeDepth,
}

impl Default for CodesConfig {
    fn default() -> Self {
        Self {
            max_depth: CodeDepth::new(0),
        }
    }
}

/// A single code entry with its subcode structure.
#[derive(Debug, Clone, Serialize)]
pub struct CodeEntry {
    /// The full code string (e.g., "AC:DI:PP").
    pub code: String,
    /// Number of occurrences.
    pub count: u64,
}

/// Per-speaker code frequency data.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerCodes {
    /// Speaker identifier.
    pub speaker: String,
    /// Code frequency entries sorted alphabetically.
    pub entries: Vec<CodeEntry>,
    /// Total codes counted.
    pub total: u64,
}

/// Typed output for the CODES command.
#[derive(Debug, Clone, Serialize)]
pub struct CodesResult {
    /// Per-speaker code frequencies.
    pub speakers: Vec<SpeakerCodes>,
    /// Total codes across all speakers.
    pub total: u64,
}

impl CodesResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("codes");
        for speaker in &self.speakers {
            let rows: Vec<TableRow> = speaker
                .entries
                .iter()
                .map(|e| TableRow {
                    values: vec![e.count.to_string(), e.code.clone()],
                })
                .collect();
            let mut section = Section::with_table(
                format!("Speaker: {}", speaker.speaker),
                vec!["Count".to_owned(), "Code".to_owned()],
                rows,
            );
            let mut fields = indexmap::IndexMap::new();
            fields.insert("Total codes".to_owned(), speaker.total.to_string());
            section.fields = fields;
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for CodesResult {
    /// Render code frequencies as a human-readable text table.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render code frequencies in CLAN-compatible format.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for speaker in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", speaker.speaker));
            for entry in &speaker.entries {
                out.push_str(&format!("{:>5}  {}\n", entry.count, entry.code));
            }
            out.push_str(&format!("Total: {}\n\n", speaker.total));
        }
        out
    }
}

/// Accumulated state for CODES across all files.
#[derive(Debug, Default)]
pub struct CodesState {
    /// Speaker → (code → count).
    speakers: BTreeMap<String, BTreeMap<String, u64>>,
}

/// CODES command implementation.
///
/// Extracts coding annotations from `%cod` dependent tiers and accumulates
/// per-speaker frequency counts. Each whitespace-delimited token on the
/// `%cod` line is treated as a separate code.
pub struct CodesCommand {
    _config: CodesConfig,
}

impl CodesCommand {
    /// Create a new CODES command with the given config.
    pub fn new(config: CodesConfig) -> Self {
        Self { _config: config }
    }
}

impl AnalysisCommand for CodesCommand {
    type Config = CodesConfig;
    type State = CodesState;
    type Output = CodesResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();

        for dep in &utterance.dependent_tiers {
            if let DependentTier::Cod(tier) = dep {
                let speaker_codes = state.speakers.entry(speaker.clone()).or_default();
                for code in cod_item_values(tier) {
                    *speaker_codes.entry(code).or_insert(0) += 1;
                }
            }
        }
    }

    fn finalize(&self, state: Self::State) -> CodesResult {
        let mut total = 0u64;
        let speakers: Vec<SpeakerCodes> = state
            .speakers
            .into_iter()
            .map(|(speaker, codes)| {
                let speaker_total: u64 = codes.values().sum();
                total += speaker_total;
                let entries: Vec<CodeEntry> = codes
                    .into_iter()
                    .map(|(code, count)| CodeEntry { code, count })
                    .collect();
                SpeakerCodes {
                    speaker,
                    entries,
                    total: speaker_total,
                }
            })
            .collect();

        CodesResult { speakers, total }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{CodTier, MainTier, Terminator, UtteranceContent, Word};

    fn make_utterance_with_cod(speaker: &str, cod_text: &str) -> Utterance {
        let content = vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        let mut utt = Utterance::new(main);
        utt.dependent_tiers
            .push(DependentTier::Cod(CodTier::from_text(cod_text)));
        utt
    }

    #[test]
    fn codes_basic() {
        let cmd = CodesCommand::new(CodesConfig::default());
        let mut state = CodesState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utterance_with_cod("CHI", "AC:DI IC:DI");
        let u2 = make_utterance_with_cod("CHI", "AC:DI");

        cmd.process_utterance(&u1, &ctx, &mut state);
        cmd.process_utterance(&u2, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.speakers.len(), 1);
        assert_eq!(result.speakers[0].total, 3);
    }

    #[test]
    fn codes_do_not_count_selectors_as_codes() {
        let cmd = CodesCommand::new(CodesConfig::default());
        let mut state = CodesState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u = make_utterance_with_cod("CHI", "<w4> $WR <w5> seep");
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        let chi = &result.speakers[0];
        assert_eq!(chi.total, 2);
        assert!(chi.entries.iter().any(|e| e.code == "$WR" && e.count == 1));
        assert!(chi.entries.iter().any(|e| e.code == "seep" && e.count == 1));
        assert!(
            !chi.entries
                .iter()
                .any(|e| e.code == "<w4>" || e.code == "<w5>")
        );
    }

    #[test]
    fn codes_empty() {
        let cmd = CodesCommand::new(CodesConfig::default());
        let state = CodesState::default();
        let result = cmd.finalize(state);
        assert_eq!(result.total, 0);
        assert!(result.speakers.is_empty());
    }
}
