//! UNIQ — Report repeated lines with frequency counts.
//!
//! Identifies and counts duplicate lines (both @header and *speaker
//! utterance lines, lowercased) across all input files. Matches CLAN
//! behavior of including all line types in the frequency table.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409094)
//! for related CLAN command specifications.
//!
//! # CLAN Equivalence
//!
//! | CLAN command               | Rust equivalent                         |
//! |----------------------------|-----------------------------------------|
//! | `uniq file.cha`            | `chatter clan uniq file.cha`            |
//! | `uniq -o file.cha`         | `chatter clan uniq file.cha --sort`     |
//!
//! # Output
//!
//! - Table of unique line texts with frequency counts (headers + utterances)
//! - Total lines processed and number of unique lines
//! - Optional frequency-descending sort (CLAN `-o` flag)
//!
//! # Differences from CLAN
//!
//! - Line identity is based on normalized rendered CHAT lines from the AST,
//!   rather than raw source text line reading.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::{Utterance, WriteChat};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
};

/// Configuration for the UNIQ command.
#[derive(Debug, Clone, Default)]
pub struct UniqConfig {
    /// Sort output by descending frequency (CLAN `-o` flag).
    pub sort_by_frequency: bool,
}

/// Per-line frequency data.
#[derive(Debug, Clone, Serialize)]
pub struct UniqEntry {
    /// The line text.
    pub text: String,
    /// Number of occurrences.
    pub count: u64,
}

/// Typed output for the UNIQ command.
#[derive(Debug, Clone, Serialize)]
pub struct UniqResult {
    /// Unique entries with frequency counts.
    pub entries: Vec<UniqEntry>,
    /// Total lines processed.
    pub total: u64,
    /// Number of unique lines.
    pub unique: u64,
}

impl UniqResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("uniq");
        let rows: Vec<TableRow> = self
            .entries
            .iter()
            .map(|e| TableRow {
                values: vec![e.count.to_string(), e.text.clone()],
            })
            .collect();

        let mut section = Section::with_table(
            "Lines".to_owned(),
            vec!["Count".to_owned(), "Text".to_owned()],
            rows,
        );
        let mut fields = indexmap::IndexMap::new();
        fields.insert("Total lines".to_owned(), self.total.to_string());
        fields.insert("Unique lines".to_owned(), self.unique.to_string());
        section.fields = fields;
        result.add_section(section);
        result
    }
}

impl CommandOutput for UniqResult {
    /// Render frequency table with total/unique counts.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible frequency list with summary line.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            out.push_str(&format!("{:>5}  {}\n", entry.count, entry.text));
        }
        out.push_str(&format!(
            "Unique number: {}    Total number: {}\n",
            self.unique, self.total
        ));
        out
    }
}

/// Accumulated state for UNIQ across all files.
///
/// Counts normalized rendered CHAT lines (all lowercased), matching the
/// command's semantic intent: repeated line texts after AST normalization.
#[derive(Debug, Default)]
pub struct UniqState {
    /// Lowercased line text → count (BTreeMap for sorted alphabetical output).
    counts: BTreeMap<String, u64>,
    /// Total lines processed (headers + utterances).
    total: u64,
}

/// UNIQ command implementation.
///
/// Accumulates lowercased rendered line text in a frequency map. The command
/// works over normalized file lines in `end_file()` because its semantic unit
/// is the serialized CHAT line, not a structural field subset.
pub struct UniqCommand {
    config: UniqConfig,
}

impl UniqCommand {
    /// Create a new UNIQ command with the given config.
    pub fn new(config: UniqConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for UniqCommand {
    type Config = UniqConfig;
    type State = UniqState;
    type Output = UniqResult;

    fn process_utterance(
        &self,
        _utterance: &Utterance,
        _file_context: &FileContext<'_>,
        _state: &mut Self::State,
    ) {
    }

    fn end_file(&self, file_context: &FileContext<'_>, state: &mut Self::State) {
        for line in file_context.chat_file.lines.iter() {
            let rendered = line.to_chat_string();
            for part in rendered.lines() {
                let lowered = part.trim().to_lowercase();
                if !lowered.is_empty() {
                    state.total += 1;
                    *state.counts.entry(lowered).or_insert(0) += 1;
                }
            }
        }
    }

    fn finalize(&self, state: Self::State) -> UniqResult {
        let unique = state.counts.len() as u64;
        let mut entries: Vec<UniqEntry> = state
            .counts
            .into_iter()
            .map(|(text, count)| UniqEntry { text, count })
            .collect();

        if self.config.sort_by_frequency {
            entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.text.cmp(&b.text)));
        }

        UniqResult {
            entries,
            total: state.total,
            unique,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{Line, MainTier, Terminator, UtteranceContent, Word};

    fn make_utterance(speaker: &str, words: &[&str]) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    #[test]
    fn uniq_basic() {
        let cmd = UniqCommand::new(UniqConfig::default());
        let mut state = UniqState::default();
        let u1 = make_utterance("CHI", &["hello", "world"]);
        let u2 = make_utterance("CHI", &["hello", "world"]);
        let u3 = make_utterance("CHI", &["goodbye"]);
        let chat_file = talkbank_model::ChatFile::new(vec![
            Line::utterance(u1.clone()),
            Line::utterance(u2.clone()),
            Line::utterance(u3.clone()),
        ]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        cmd.process_utterance(&u1, &ctx, &mut state);
        cmd.process_utterance(&u2, &ctx, &mut state);
        cmd.process_utterance(&u3, &ctx, &mut state);
        cmd.end_file(&ctx, &mut state);

        let result = cmd.finalize(state);
        // 3 utterance lines + 0 header lines (empty ChatFile)
        assert_eq!(result.total, 3);
        assert_eq!(result.unique, 2);
    }

    #[test]
    fn uniq_sort_by_frequency() {
        let cmd = UniqCommand::new(UniqConfig {
            sort_by_frequency: true,
        });
        let mut state = UniqState::default();
        let u1 = make_utterance("CHI", &["a"]);
        let u2 = make_utterance("CHI", &["b"]);
        let u3 = make_utterance("CHI", &["b"]);
        let u4 = make_utterance("CHI", &["b"]);
        let chat_file = talkbank_model::ChatFile::new(vec![
            Line::utterance(u1.clone()),
            Line::utterance(u2.clone()),
            Line::utterance(u3.clone()),
            Line::utterance(u4.clone()),
        ]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        cmd.process_utterance(&u1, &ctx, &mut state);
        cmd.process_utterance(&u2, &ctx, &mut state);
        cmd.process_utterance(&u3, &ctx, &mut state);
        cmd.process_utterance(&u4, &ctx, &mut state);
        cmd.end_file(&ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.entries[0].count, 3); // "*chi:\tb ." first (higher frequency)
        assert_eq!(result.entries[1].count, 1); // "*chi:\ta ." second
    }

    #[test]
    fn uniq_empty() {
        let cmd = UniqCommand::new(UniqConfig::default());
        let state = UniqState::default();
        let result = cmd.finalize(state);
        assert_eq!(result.total, 0);
        assert_eq!(result.unique, 0);
        assert!(result.entries.is_empty());
    }
}
