//! FREQPOS — Word frequency by position in utterance.
//!
//! Reimplements CLAN's FREQPOS command, which counts how often each word
//! appears in initial, final, other (middle), or one-word positions within
//! utterances. FREQPOS is part of the FREQ family of commands and is useful
//! for studying positional word preferences -- for example, whether a child
//! tends to place certain words at the beginning or end of utterances.
//!
//! Position classification rules:
//! - **Initial**: first word of a multi-word utterance
//! - **Final**: last word of a multi-word utterance
//! - **Other**: any middle word of a multi-word utterance (3+ words)
//! - **One-word**: the sole word in a single-word utterance
//!
//! # CLAN Equivalence
//!
//! | CLAN command                | Rust equivalent                           |
//! |-----------------------------|-------------------------------------------|
//! | `freqpos file.cha`          | `chatter analyze freqpos file.cha`        |
//! | `freqpos +t*CHI file.cha`   | `chatter analyze freqpos file.cha -s CHI` |
//!
//! # Output
//!
//! Global word list (sorted alphabetically by display form) with positional
//! breakdown (initial/final/other/one-word counts per word), followed by
//! aggregate position totals.
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Position classification operates on parsed AST word lists rather than
//!   raw text token splitting.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::word_filter::countable_words;
use crate::framework::{
    AnalysisCommand, CommandOutput, FileContext, NormalizedWord, clan_display_form,
};

/// Configuration for the FREQPOS command.
#[derive(Debug, Clone, Default)]
pub struct FreqposConfig {}

/// Positional counts for a single word.
#[derive(Debug, Default, Clone)]
struct WordPositionCounts {
    /// Total occurrences
    total: u64,
    /// Occurrences as first word of a multi-word utterance
    initial: u64,
    /// Occurrences as last word of a multi-word utterance
    final_pos: u64,
    /// Occurrences in middle positions of a multi-word utterance
    other: u64,
    /// Occurrences as the sole word in a one-word utterance
    one_word: u64,
    /// CLAN display form (preserves `+` in compounds)
    display_form: String,
}

/// A single word position entry in the output.
#[derive(Debug, Clone, Serialize)]
pub struct FreqposEntry {
    /// The word (normalized).
    pub word: String,
    /// CLAN display form.
    pub display_form: String,
    /// Total occurrences.
    pub total: u64,
    /// Occurrences in initial position.
    pub initial: u64,
    /// Occurrences in final position.
    pub final_pos: u64,
    /// Occurrences in other (middle) positions.
    pub other: u64,
    /// Occurrences as one-word utterance.
    pub one_word: u64,
}

/// Typed output for the FREQPOS command.
#[derive(Debug, Clone, Serialize)]
pub struct FreqposResult {
    /// Word entries sorted alphabetically by display form.
    pub entries: Vec<FreqposEntry>,
    /// Total words in initial position across all entries.
    pub total_initial: u64,
    /// Total words in other (middle) position.
    pub total_other: u64,
    /// Total words in final position.
    pub total_final: u64,
    /// Total one-word utterances.
    pub total_one_word: u64,
}

impl CommandOutput for FreqposResult {
    /// Use CLAN-aligned text as the default textual representation.
    fn render_text(&self) -> String {
        self.render_clan()
    }

    /// CLAN-compatible output matching legacy CLAN character-for-character.
    ///
    /// Format:
    /// ```text
    ///   1  cookie               initial =  0, final =  1, other =  0, one word =  0
    ///
    /// Number of words in an initial position =  3
    /// Number of words in an other position   =  6
    /// Number of words in a final position    =  3
    /// Number of one word utterences          =  1
    /// ```
    fn render_clan(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        // Find the max display form length for alignment
        let max_display_len = self
            .entries
            .iter()
            .map(|e| e.display_form.len())
            .max()
            .unwrap_or(0)
            .max(21);

        for entry in &self.entries {
            writeln!(
                out,
                "{:>3}  {:<width$} initial = {:>2}, final = {:>2}, other = {:>2}, one word = {:>2}",
                entry.total,
                entry.display_form,
                entry.initial,
                entry.final_pos,
                entry.other,
                entry.one_word,
                width = max_display_len,
            )
            .ok();
        }

        // Position summary footer
        writeln!(out).ok();
        writeln!(
            out,
            "Number of words in an initial position = {:>2}",
            self.total_initial
        )
        .ok();
        writeln!(
            out,
            "Number of words in an other position   = {:>2}",
            self.total_other
        )
        .ok();
        writeln!(
            out,
            "Number of words in a final position    = {:>2}",
            self.total_final
        )
        .ok();
        writeln!(
            out,
            "Number of one word utterences          = {:>2}",
            self.total_one_word
        )
        .ok();

        out
    }
}

/// Accumulated state for FREQPOS across all files.
#[derive(Debug, Default)]
pub struct FreqposState {
    /// Per-word position counts, keyed by normalized word.
    by_word: BTreeMap<NormalizedWord, WordPositionCounts>,
}

/// FREQPOS command implementation.
///
/// For each utterance, classifies each word by its position
/// (initial/final/other/one-word) and accumulates counts globally.
#[derive(Debug, Clone, Default)]
pub struct FreqposCommand;

impl AnalysisCommand for FreqposCommand {
    type Config = FreqposConfig;
    type State = FreqposState;
    type Output = FreqposResult;

    /// Classify each lexical token by utterance position and accumulate counts.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Collect words with their display forms
        let words: Vec<(NormalizedWord, String)> = countable_words(&utterance.main.content.content)
            .map(|w| (NormalizedWord::from_word(w), clan_display_form(w)))
            .collect();

        let len = words.len();
        if len == 0 {
            return;
        }

        for (i, (key, display)) in words.iter().enumerate() {
            let entry = state.by_word.entry(key.clone()).or_default();
            if entry.display_form.is_empty() {
                entry.display_form.clone_from(display);
            }
            entry.total += 1;

            if len == 1 {
                entry.one_word += 1;
            } else if i == 0 {
                entry.initial += 1;
            } else if i == len - 1 {
                entry.final_pos += 1;
            } else {
                entry.other += 1;
            }
        }
    }

    /// Build sorted entries and compute global position totals.
    fn finalize(&self, state: Self::State) -> FreqposResult {
        let mut total_initial: u64 = 0;
        let mut total_other: u64 = 0;
        let mut total_final: u64 = 0;
        let mut total_one_word: u64 = 0;

        // Sort by display form alphabetically
        let mut entries_vec: Vec<(NormalizedWord, WordPositionCounts)> =
            state.by_word.into_iter().collect();
        entries_vec.sort_by(|a, b| a.1.display_form.cmp(&b.1.display_form));

        let entries: Vec<FreqposEntry> = entries_vec
            .into_iter()
            .map(|(key, counts)| {
                total_initial += counts.initial;
                total_other += counts.other;
                total_final += counts.final_pos;
                total_one_word += counts.one_word;

                FreqposEntry {
                    word: key.as_str().to_owned(),
                    display_form: counts.display_form,
                    total: counts.total,
                    initial: counts.initial,
                    final_pos: counts.final_pos,
                    other: counts.other,
                    one_word: counts.one_word,
                }
            })
            .collect();

        FreqposResult {
            entries,
            total_initial,
            total_other,
            total_final,
            total_one_word,
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

    /// Build a stable `FileContext` fixture reused by command tests.
    fn file_ctx(chat_file: &talkbank_model::ChatFile) -> FileContext<'_> {
        FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file,
            filename: "test",
            line_map: None,
        }
    }

    /// Multi-word utterances should split counts across initial/other/final buckets.
    #[test]
    fn freqpos_position_tracking() {
        let command = FreqposCommand;
        let mut state = FreqposState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        // "I want cookie" → I=initial, want=other, cookie=final
        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.total_initial, 1);
        assert_eq!(result.total_other, 1);
        assert_eq!(result.total_final, 1);
        assert_eq!(result.total_one_word, 0);
    }

    /// Single-token utterances should increment only the one-word bucket.
    #[test]
    fn freqpos_one_word_utterance() {
        let command = FreqposCommand;
        let mut state = FreqposState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        let u = make_utterance("CHI", &["hello"]);
        command.process_utterance(&u, &ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.total_one_word, 1);
        assert_eq!(result.total_initial, 0);
    }

    /// Finalizing untouched state should produce empty entries and zero totals.
    #[test]
    fn freqpos_empty_state() {
        let command = FreqposCommand;
        let state = FreqposState::default();
        let result = command.finalize(state);
        assert!(result.entries.is_empty());
    }
}
