//! MAXWD — Longest Words.
//!
//! Finds the longest words used by each speaker, reporting a ranked table
//! of unique words sorted by character length descending. Word length is
//! measured in characters after normalization (lowercasing, stripping `+`
//! and `'` for CLAN compatibility).
//!
//! MAXWD does not have a dedicated section in the CLAN manual.
//!
//! # CLAN Equivalence
//!
//! | CLAN command               | Rust equivalent                          |
//! |----------------------------|------------------------------------------|
//! | `maxwd file.cha`           | `chatter analyze maxwd file.cha`         |
//! | `maxwd +t*CHI file.cha`    | `chatter analyze maxwd file.cha -s CHI`  |
//!
//! # Output
//!
//! Per speaker:
//! - Table of longest words sorted by length descending (up to `limit`)
//! - Maximum word length
//! - Mean word length
//! - Total and unique word counts
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Word length measurement uses parsed, normalized word content rather
//!   than raw text character counting.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{SpeakerCode, Utterance};

use crate::framework::word_filter::countable_words;
use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, NormalizedWord, OutputFormat,
    Section, TableRow, WordCount, WordLimit, clan_display_form,
};

/// Configuration for the MAXWD command.
#[derive(Debug, Clone)]
pub struct MaxwdConfig {
    /// Maximum number of words to show in the output table.
    /// Default: 20
    pub limit: WordLimit,
}

impl Default for MaxwdConfig {
    /// Default to CLAN-style top-20 longest words.
    fn default() -> Self {
        Self {
            limit: WordLimit::new(20),
        }
    }
}

/// A single occurrence of a longest word, with its line number.
#[derive(Debug, Clone, Serialize)]
pub struct MaxwdOccurrence {
    /// The display form of the word (preserving `+` in compounds).
    pub display_form: String,
    /// Character length (CLAN-style: excluding `+` and `'`).
    pub char_length: usize,
    /// 1-based line number in the source file.
    pub line_number: usize,
}

/// Per-speaker longest-word results.
#[derive(Debug, Clone, Serialize)]
pub struct MaxwdSpeakerResult {
    /// Speaker code.
    pub speaker: String,
    /// Length of the longest word.
    pub max_length: usize,
    /// Mean word length across all tokens.
    pub mean_length: f64,
    /// Total word tokens counted.
    pub total_words: WordCount,
    /// Number of unique words encountered.
    pub unique_words: usize,
    /// Top words sorted by length descending: `(length, word)`.
    pub top_words: Vec<(usize, String)>,
    /// CLAN display forms (preserving `+` in compounds), keyed by normalized word.
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub display_forms: std::collections::HashMap<String, String>,
    /// Line numbers for words (for CLAN format), keyed by normalized word.
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub line_numbers: std::collections::HashMap<String, usize>,
}

/// Typed output for the MAXWD command.
#[derive(Debug, Clone, Serialize)]
pub struct MaxwdResult {
    /// Per-speaker longest-word results.
    pub speakers: Vec<MaxwdSpeakerResult>,
    /// All occurrences of the globally longest word(s), sorted by line number.
    /// Used by `render_clan()` to match CLAN's output of every tied occurrence.
    pub longest_occurrences: Vec<MaxwdOccurrence>,
}

impl MaxwdResult {
    /// Convert typed MAXWD output into the shared section/table render model.
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("maxwd");
        for data in &self.speakers {
            let mut fields = IndexMap::new();
            fields.insert("Max word length".to_owned(), data.max_length.to_string());
            fields.insert(
                "Mean word length".to_owned(),
                format!("{:.3}", data.mean_length),
            );
            fields.insert("Total words".to_owned(), data.total_words.to_string());
            fields.insert("Unique words".to_owned(), data.unique_words.to_string());

            let rows: Vec<TableRow> = data
                .top_words
                .iter()
                .map(|(len, word)| TableRow {
                    values: vec![len.to_string(), word.clone()],
                })
                .collect();

            let mut section = Section::with_table(
                format!("Speaker: {}", data.speaker),
                vec!["Length".to_owned(), "Word".to_owned()],
                rows,
            );
            section.fields = fields;
            result.add_section(section);
        }
        result
    }
}

/// Count characters the way CLAN does: strip `+` and `'` before counting.
fn clan_char_count(word: &str) -> usize {
    word.chars().filter(|c| *c != '+' && *c != '\'').count()
}

impl CommandOutput for MaxwdResult {
    /// Render via the shared tabular text formatter.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// CLAN-compatible output matching legacy CLAN character-for-character.
    ///
    /// CLAN prints EVERY occurrence of words tied for the longest length,
    /// each with its line number, sorted by line number. Words are NOT
    /// deduplicated — if the same word appears on two different lines,
    /// both instances are listed.
    ///
    /// Format (from CLAN snapshot):
    /// ```text
    /// *** File "pipeout": line 22: 9 characters long:
    /// choo+choo's
    /// ```
    fn render_clan(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        for occ in &self.longest_occurrences {
            writeln!(
                out,
                "*** File \"pipeout\": line {}: {} characters long:",
                occ.line_number, occ.char_length
            )
            .ok();
            writeln!(out, "{}", occ.display_form).ok();
        }

        out
    }
}

/// Per-speaker word tracking for finding longest words.
#[derive(Debug, Default)]
struct SpeakerMaxwd {
    /// All unique words encountered, keyed by normalized text,
    /// storing character length.
    /// Using BTreeMap for deterministic iteration order.
    words: BTreeMap<NormalizedWord, usize>,
    /// CLAN display forms (preserving `+` in compounds)
    display_forms: std::collections::HashMap<NormalizedWord, String>,
    /// Total characters across all word tokens (for mean)
    total_chars: u64,
    /// Total word tokens counted
    total_words: WordCount,
}

/// Accumulated state for MAXWD across all files.
#[derive(Debug, Default)]
pub struct MaxwdState {
    /// Per-speaker word data, keyed by speaker code
    by_speaker: IndexMap<SpeakerCode, SpeakerMaxwd>,
    /// Word → line number mapping for CLAN format (first occurrence)
    word_line_numbers: std::collections::HashMap<NormalizedWord, usize>,
    /// Every word occurrence: (display_form, char_length, line_number).
    /// Not deduplicated — used to find all occurrences at the max length.
    all_occurrences: Vec<(String, usize, usize)>,
}

/// MAXWD command implementation.
///
/// Collects unique words per speaker, then reports the longest ones
/// sorted by character length descending.
#[derive(Debug, Clone, Default)]
pub struct MaxwdCommand {
    config: MaxwdConfig,
}

impl MaxwdCommand {
    /// Create a MAXWD command with the given configuration.
    pub fn new(config: MaxwdConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for MaxwdCommand {
    type Config = MaxwdConfig;
    type State = MaxwdState;
    type Output = MaxwdResult;

    /// Accumulate per-speaker lexical inventory, lengths, and first-seen line numbers.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Arc<str> clone — cheap atomic ref-count increment, no allocation
        let speaker = utterance.main.speaker.clone();
        let speaker_data = state
            .by_speaker
            .entry(speaker)
            .or_insert_with(SpeakerMaxwd::default);

        // Compute line number: O(log n) via LineMap when available, else 0
        let line_number = file_context
            .line_map
            .map(|lm| lm.line_of(utterance.main.span.start))
            .unwrap_or(0);

        for word in countable_words(&utterance.main.content.content) {
            let text = NormalizedWord::from_word(word);
            let len = text.as_str().chars().count();
            let display = clan_display_form(word);
            let clan_len = clan_char_count(&display);

            // Track unique word → length (keep the word for display)
            speaker_data.words.entry(text.clone()).or_insert(len);
            speaker_data
                .display_forms
                .entry(text.clone())
                .or_insert_with(|| display.clone());
            state.word_line_numbers.entry(text).or_insert(line_number);

            // Track every occurrence for CLAN output (not deduplicated)
            state.all_occurrences.push((display, clan_len, line_number));

            speaker_data.total_chars += len as u64;
            speaker_data.total_words += 1;
        }
    }

    /// Build per-speaker longest-word tables and summary metrics.
    fn finalize(&self, state: Self::State) -> MaxwdResult {
        let mut speakers = Vec::new();
        for (speaker, data) in state.by_speaker {
            if data.total_words == 0 {
                continue;
            }

            let mut entries: Vec<(NormalizedWord, usize)> = data.words.into_iter().collect();
            entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

            let max_length = entries.first().map(|(_, len)| *len).unwrap_or(0);
            let unique_words = entries.len();
            let mean_length = data.total_chars as f64 / data.total_words as f64;

            let top_words: Vec<(usize, String)> = entries
                .into_iter()
                .take(self.config.limit.get())
                .map(|(word, len)| (len, word.as_str().to_owned()))
                .collect();

            // Build display_forms and line_numbers maps keyed by normalized word string
            let display_forms: std::collections::HashMap<String, String> = data
                .display_forms
                .into_iter()
                .map(|(k, v)| (k.as_str().to_owned(), v))
                .collect();
            let line_numbers: std::collections::HashMap<String, usize> = state
                .word_line_numbers
                .iter()
                .map(|(k, v)| (k.as_str().to_owned(), *v))
                .collect();

            speakers.push(MaxwdSpeakerResult {
                speaker: speaker.as_str().to_owned(),
                max_length,
                mean_length,
                total_words: data.total_words,
                unique_words,
                top_words,
                display_forms,
                line_numbers,
            });
        }
        // Find the global max CLAN char length across all occurrences
        let global_max = state
            .all_occurrences
            .iter()
            .map(|(_, len, _)| *len)
            .max()
            .unwrap_or(0);

        // Collect all occurrences at the max length, sorted by line number
        let mut longest_occurrences: Vec<MaxwdOccurrence> = state
            .all_occurrences
            .into_iter()
            .filter(|(_, len, _)| *len == global_max && global_max > 0)
            .map(|(display_form, char_length, line_number)| MaxwdOccurrence {
                display_form,
                char_length,
                line_number,
            })
            .collect();
        longest_occurrences.sort_by_key(|o| o.line_number);

        MaxwdResult {
            speakers,
            longest_occurrences,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{MainTier, Terminator, UtteranceContent, Word};

    /// Build a minimal utterance with plain lexical tokens for tests.
    fn make_utterance(speaker: &str, words: &[&str]) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    /// Longest lexical item should surface first with its character count.
    #[test]
    fn maxwd_finds_longest_words() {
        let command = MaxwdCommand::default();
        let mut state = MaxwdState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utterance("CHI", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["hippopotamus", "is", "big"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 1);

        let chi = &result.speakers[0];
        assert_eq!(chi.max_length, 12); // hippopotamus
        // First entry should be the longest word
        assert_eq!(chi.top_words[0].1, "hippopotamus");
        assert_eq!(chi.top_words[0].0, 12);
    }

    /// Configured output limit should cap number of reported longest words.
    #[test]
    fn maxwd_respects_limit() {
        let config = MaxwdConfig {
            limit: crate::framework::WordLimit::new(2),
        };
        let command = MaxwdCommand::new(config);
        let mut state = MaxwdState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u = make_utterance("CHI", &["a", "bb", "ccc", "dddd", "eeeee"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        let result = command.finalize(state);
        let chi = &result.speakers[0];
        // Only top 2 longest words shown
        assert_eq!(chi.top_words.len(), 2);
        assert_eq!(chi.top_words[0].1, "eeeee");
        assert_eq!(chi.top_words[1].1, "dddd");
    }

    /// Repeated tokens should increment totals but keep one unique-word entry.
    #[test]
    fn maxwd_deduplicates_words() {
        let command = MaxwdCommand::default();
        let mut state = MaxwdState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // Same word repeated — should appear once in output
        let u = make_utterance("CHI", &["cookie", "cookie", "cookie"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        let result = command.finalize(state);
        let chi = &result.speakers[0];
        assert_eq!(chi.unique_words, 1);
        assert_eq!(chi.total_words, 3);
        assert_eq!(chi.top_words.len(), 1);
    }

    /// Finalizing untouched state should return no speaker sections.
    #[test]
    fn maxwd_empty_state() {
        let command = MaxwdCommand::default();
        let state = MaxwdState::default();

        let result = command.finalize(state);
        assert!(result.speakers.is_empty());
    }
}
