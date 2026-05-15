//! COOCCUR — Word co-occurrence (bigram) counting.
//!
//! Reimplements CLAN's COOCCUR command, which counts adjacent word pairs
//! (bigrams) across utterances. For each utterance, every pair of consecutive
//! countable words is recorded as a directed bigram. Pairs are directional:
//! ("put", "the") and ("the", "put") are counted separately.
//!
//! COOCCUR is part of the FREQ family of commands and is useful for studying
//! word collocations and sequential patterns in speech.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                         | Rust equivalent                                       |
//! |--------------------------------------|-------------------------------------------------------|
//! | `cooccur file.cha`                   | `chatter analyze cooccur file.cha`                    |
//! | `cooccur +t*CHI file.cha`            | `chatter analyze cooccur file.cha -s CHI`             |
//!
//! # Output
//!
//! - Table of adjacent word pairs with co-occurrence counts
//! - Default sort: by frequency descending, then alphabetically
//! - CLAN output: sorted alphabetically by pair display form
//! - Summary: unique pair count, total pair instances, total utterances
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Bigram extraction operates on parsed AST content rather than raw text.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::word_filter::countable_words;
use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, NormalizedWord, OutputFormat,
    Section, TableRow, UtteranceCount, clan_display_form,
};

/// Configuration for the COOCCUR command.
#[derive(Debug, Clone, Default)]
pub struct CooccurConfig {}

/// A co-occurring adjacent word pair with its frequency count.
#[derive(Debug, Clone, Serialize)]
pub struct CooccurPair {
    /// First word in the pair (as it appears in utterance order).
    pub word1: String,
    /// Second word in the pair (adjacent to word1).
    pub word2: String,
    /// CLAN display form of word1 (preserves `+` in compounds).
    pub display1: String,
    /// CLAN display form of word2.
    pub display2: String,
    /// Number of times this adjacent pair occurs.
    pub count: u64,
}

/// Typed output for the COOCCUR command.
#[derive(Debug, Clone, Serialize)]
pub struct CooccurResult {
    /// Word pairs sorted by co-occurrence count descending.
    pub pairs: Vec<CooccurPair>,
    /// Number of unique word pairs observed.
    pub unique_pairs: usize,
    /// Sum of all pair counts.
    pub total_pair_instances: u64,
    /// Total utterances examined.
    pub total_utterances: UtteranceCount,
}

impl CooccurResult {
    /// Convert typed co-occurrence data into the shared section/table render model.
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("cooccur");
        if self.pairs.is_empty() {
            return result;
        }

        let rows: Vec<TableRow> = self
            .pairs
            .iter()
            .map(|p| TableRow {
                values: vec![p.word1.clone(), p.word2.clone(), p.count.to_string()],
            })
            .collect();

        let mut section = Section::with_table(
            "Co-occurrences".to_owned(),
            vec!["Word 1".to_owned(), "Word 2".to_owned(), "Count".to_owned()],
            rows,
        );
        section
            .fields
            .insert("Unique pairs".to_owned(), self.unique_pairs.to_string());
        section.fields.insert(
            "Total pair instances".to_owned(),
            self.total_pair_instances.to_string(),
        );
        section.fields.insert(
            "Total utterances".to_owned(),
            self.total_utterances.to_string(),
        );

        result.add_section(section);
        result
    }
}

impl CommandOutput for CooccurResult {
    /// Render via the shared tabular text formatter.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// CLAN-compatible output matching legacy CLAN character-for-character.
    ///
    /// Format:
    /// ```text
    ///   1  gonna put
    ///   1  more cookie
    ///   1  the choo+choo's
    /// ```
    fn render_clan(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        // CLAN sorts alphabetically by pair display form
        let mut sorted: Vec<&CooccurPair> = self.pairs.iter().collect();
        sorted.sort_by(|a, b| (&a.display1, &a.display2).cmp(&(&b.display1, &b.display2)));

        for pair in &sorted {
            writeln!(
                out,
                "{:>3}  {} {}",
                pair.count, pair.display1, pair.display2
            )
            .ok();
        }

        out
    }
}

/// An ordered word pair used as a map key for adjacent bigrams.
///
/// Pairs preserve utterance order: ("put", "the") represents "put" followed
/// by "the" in the utterance. This matches CLAN's behavior where
/// ("put", "the") and ("the", "put") are distinct pairs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct WordPair(NormalizedWord, NormalizedWord);

impl WordPair {
    /// Create a pair preserving utterance order (first, second).
    fn ordered(a: NormalizedWord, b: NormalizedWord) -> Self {
        WordPair(a, b)
    }

    /// Test-only helper for constructing ordered pair keys from string literals.
    #[cfg(test)]
    fn new(a: &str, b: &str) -> Self {
        let a = NormalizedWord(a.to_owned());
        let b = NormalizedWord(b.to_owned());
        Self::ordered(a, b)
    }
}

/// Display form for a word pair.
#[derive(Debug, Clone)]
struct PairDisplay {
    display1: String,
    display2: String,
}

/// Per-pair accumulated data: count and display forms.
#[derive(Debug, Clone)]
struct PairData {
    count: u64,
    display: PairDisplay,
}

/// Accumulated state for COOCCUR across all files.
#[derive(Debug, Default)]
pub struct CooccurState {
    /// Co-occurrence data for each adjacent word pair (merged counts + display forms).
    pairs: BTreeMap<WordPair, PairData>,
    /// Total utterances examined.
    pub total_utterances: UtteranceCount,
}

/// COOCCUR command implementation.
///
/// For each utterance, extracts countable words and counts adjacent pairs
/// (bigrams), matching CLAN's behavior.
#[derive(Debug, Clone, Default)]
pub struct CooccurCommand;

impl AnalysisCommand for CooccurCommand {
    type Config = CooccurConfig;
    type State = CooccurState;
    type Output = CooccurResult;

    /// Count adjacent lexical bigrams from the current utterance.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        state.total_utterances += 1;

        // Collect words with their display forms
        let words: Vec<(NormalizedWord, String)> = countable_words(&utterance.main.content.content)
            .map(|w| (NormalizedWord::from_word(w), clan_display_form(w)))
            .collect();

        // Count adjacent pairs (bigrams) preserving utterance order.
        // Single map lookup per pair avoids cloning the key for a second map.
        for window in words.windows(2) {
            let (ref key_a, ref display_a) = window[0];
            let (ref key_b, ref display_b) = window[1];

            let pair = WordPair::ordered(key_a.clone(), key_b.clone());

            state
                .pairs
                .entry(pair)
                .and_modify(|data| data.count += 1)
                .or_insert_with(|| PairData {
                    count: 1,
                    display: PairDisplay {
                        display1: display_a.clone(),
                        display2: display_b.clone(),
                    },
                });
        }
    }

    /// Materialize sorted output rows and aggregate totals from map state.
    fn finalize(&self, state: Self::State) -> CooccurResult {
        if state.pairs.is_empty() {
            return CooccurResult {
                pairs: Vec::new(),
                unique_pairs: 0,
                total_pair_instances: 0,
                total_utterances: state.total_utterances,
            };
        }

        let unique_pairs = state.pairs.len();
        let total_pair_instances: u64 = state.pairs.values().map(|d| d.count).sum();

        // Sort pairs by frequency (descending), then alphabetically
        let mut sorted: Vec<(WordPair, PairData)> = state.pairs.into_iter().collect();
        sorted.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));

        let pairs: Vec<CooccurPair> = sorted
            .into_iter()
            .map(|(pair, data)| CooccurPair {
                word1: pair.0.as_str().to_owned(),
                word2: pair.1.as_str().to_owned(),
                display1: data.display.display1,
                display2: data.display.display2,
                count: data.count,
            })
            .collect();

        CooccurResult {
            pairs,
            unique_pairs,
            total_pair_instances,
            total_utterances: state.total_utterances,
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

    /// Adjacent tokens should produce one ordered bigram per sliding window.
    #[test]
    fn cooccur_adjacent_pairs() {
        let command = CooccurCommand;
        let mut state = CooccurState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        // "I want cookie" → adjacent pairs: (i, want), (want, cookie) — utterance order
        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &ctx, &mut state);

        // Should have 2 adjacent pairs in utterance order
        assert_eq!(state.pairs.len(), 2);
        assert_eq!(state.pairs[&WordPair::new("i", "want")].count, 1);
        assert_eq!(state.pairs[&WordPair::new("want", "cookie")].count, 1);
    }

    /// Pair counts should accumulate across multiple utterances.
    #[test]
    fn cooccur_accumulates_across_utterances() {
        let command = CooccurCommand;
        let mut state = CooccurState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        let u1 = make_utterance("CHI", &["I", "want"]);
        let u2 = make_utterance("CHI", &["I", "want", "more"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);

        // (i, want) should have count 2
        assert_eq!(state.pairs[&WordPair::new("i", "want")].count, 2);
        assert_eq!(state.total_utterances, 2);
    }

    /// One-token utterances should not emit any pair entries.
    #[test]
    fn cooccur_single_word_no_pairs() {
        let command = CooccurCommand;
        let mut state = CooccurState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        let u = make_utterance("CHI", &["hello"]);
        command.process_utterance(&u, &ctx, &mut state);

        assert_eq!(state.pairs.len(), 0);
    }

    /// Finalizing untouched state should return an empty result set.
    #[test]
    fn cooccur_empty_state() {
        let command = CooccurCommand;
        let state = CooccurState::default();
        let result = command.finalize(state);
        assert!(result.pairs.is_empty());
    }

    /// Pair keys are directional: `(a,b)` and `(b,a)` are distinct.
    #[test]
    fn word_pair_preserves_utterance_order() {
        // Utterance-order pairs: (want, cookie) ≠ (cookie, want)
        let p1 = WordPair::new("want", "cookie");
        let p2 = WordPair::new("cookie", "want");
        assert_ne!(p1, p2);
        assert_eq!(p1.0.as_str(), "want");
        assert_eq!(p1.1.as_str(), "cookie");
    }
}
