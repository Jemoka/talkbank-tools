//! WDSIZE — Word size (character length) histogram from `%mor` tier stems.
//!
//! By default WDSIZE uses the `%mor` tier to extract word stems and counts
//! their character lengths. This differs from WDLEN which counts main tier
//! word lengths. When `%mor` is unavailable, falls back to main tier words.
//!
//! Output: character-length histogram per speaker with mean word size.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) for the
//! original WDSIZE command specification.
//!
//! # Differences from CLAN
//!
//! - Uses typed `MorTier` items with `MorWord.lemma` rather than raw string
//!   parsing of `%mor` tier text.
//! - Compound words concatenate all compound lemmas (matching CLAN behavior).
//! - Supports JSON and CSV output in addition to text/XLS.
//! - Optional `--main-tier` flag to count main tier words instead of stems.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::word_filter::{countable_words, has_countable_words};
use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    WordCount,
};

/// Configuration for the WDSIZE command.
#[derive(Debug, Clone, Default)]
pub struct WdsizeConfig {
    /// Use main tier words instead of `%mor` stems.
    pub use_main_tier: bool,
}

/// Per-speaker word size distribution.
#[derive(Debug, Clone, Serialize)]
pub struct WdsizeDistribution {
    /// Speaker identifier.
    pub speaker: String,
    /// Character length -> count mapping.
    pub distribution: BTreeMap<usize, u64>,
    /// Total number of words measured.
    pub total_words: WordCount,
    /// Sum of all character lengths.
    pub total_chars: u64,
}

impl WdsizeDistribution {
    /// Mean word size in characters.
    fn mean(&self) -> f64 {
        if self.total_words == 0 {
            0.0
        } else {
            self.total_chars as f64 / self.total_words as f64
        }
    }
}

/// Result of the WDSIZE command.
#[derive(Debug, Clone, Serialize)]
pub struct WdsizeResult {
    /// Per-speaker distributions.
    pub speakers: Vec<WdsizeDistribution>,
}

impl CommandOutput for WdsizeResult {
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    fn render_clan(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "\nNumber of words of each length in characters");

        let max_len = self
            .speakers
            .iter()
            .flat_map(|d| d.distribution.keys())
            .copied()
            .max()
            .unwrap_or(1);

        let col_width = 5;
        let max_speaker_label = self
            .speakers
            .iter()
            .map(|d| d.speaker.len() + 2)
            .max()
            .unwrap_or(0);
        let label_width = "lengths".len().max(max_speaker_label) + 1;

        let mut header = format!("{:<label_width$}", "lengths");
        for col in 1..=max_len {
            let _ = write!(header, "{:>col_width$}", col);
        }
        let _ = write!(header, "{:>7}", "Mean");
        let _ = writeln!(out, "{header}");

        for dist in self.speakers.iter().rev() {
            let speaker_label = format!("*{}:", dist.speaker);
            let mut row = format!("{:<label_width$}", speaker_label);
            for col in 1..=max_len {
                let count = dist.distribution.get(&col).copied().unwrap_or(0);
                let _ = write!(row, "{:>col_width$}", count);
            }
            let _ = write!(row, "{:>7.3}", dist.mean());
            let _ = writeln!(out, "{row}");
        }
        out
    }
}

impl WdsizeResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("wdsize");
        for data in &self.speakers {
            let rows: Vec<TableRow> = data
                .distribution
                .iter()
                .map(|(length, count)| TableRow {
                    values: vec![length.to_string(), count.to_string()],
                })
                .collect();

            let mut section = Section::with_table(
                format!("Speaker: {}", data.speaker),
                vec!["Length".to_owned(), "Count".to_owned()],
                rows,
            );
            let mut fields = indexmap::IndexMap::new();
            fields.insert("Total words".to_owned(), data.total_words.to_string());
            fields.insert("Mean word size".to_owned(), format!("{:.3}", data.mean()));
            section.fields = fields;
            result.add_section(section);
        }
        result
    }
}

/// Per-speaker accumulator.
#[derive(Debug, Default)]
struct SpeakerAccum {
    distribution: BTreeMap<usize, u64>,
    total_words: u64,
    total_chars: u64,
}

impl SpeakerAccum {
    fn record(&mut self, char_len: usize) {
        *self.distribution.entry(char_len).or_insert(0) += 1;
        self.total_words += 1;
        self.total_chars += char_len as u64;
    }

    fn into_distribution(self, speaker: &str) -> WdsizeDistribution {
        WdsizeDistribution {
            speaker: speaker.to_owned(),
            distribution: self.distribution,
            total_words: self.total_words,
            total_chars: self.total_chars,
        }
    }
}

/// Accumulated state for WDSIZE.
#[derive(Debug, Default)]
pub struct WdsizeState {
    by_speaker: indexmap::IndexMap<String, SpeakerAccum>,
}

/// WDSIZE command implementation.
#[derive(Debug, Clone)]
pub struct WdsizeCommand {
    config: WdsizeConfig,
}

impl WdsizeCommand {
    /// Create a new WDSIZE command with configuration.
    pub fn new(config: WdsizeConfig) -> Self {
        Self { config }
    }
}

impl Default for WdsizeCommand {
    fn default() -> Self {
        Self::new(WdsizeConfig::default())
    }
}

impl AnalysisCommand for WdsizeCommand {
    type Config = WdsizeConfig;
    type State = WdsizeState;
    type Output = WdsizeResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        if !has_countable_words(&utterance.main.content.content) {
            return;
        }

        let speaker = utterance.main.speaker.to_string();
        let accum = state.by_speaker.entry(speaker).or_default();

        if self.config.use_main_tier {
            // Count main tier word character lengths
            for word in countable_words(&utterance.main.content.content) {
                let char_len = word.cleaned_text().chars().count();
                accum.record(char_len);
            }
        } else if let Some(mor_tier) = utterance.mor_tier() {
            // Count %mor lemma character lengths (default behavior)
            for mor_item in mor_tier.items().iter() {
                let char_len = mor_item.main.lemma.as_str().chars().count();
                accum.record(char_len);

                // Count clitic lemmas separately
                for clitic in &mor_item.post_clitics {
                    let char_len = clitic.lemma.as_str().chars().count();
                    accum.record(char_len);
                }
            }
        } else {
            // Fallback to main tier if no %mor
            for word in countable_words(&utterance.main.content.content) {
                let char_len = word.cleaned_text().chars().count();
                accum.record(char_len);
            }
        }
    }

    fn finalize(&self, state: Self::State) -> WdsizeResult {
        let speakers: Vec<_> = state
            .by_speaker
            .into_iter()
            .map(|(speaker, accum)| accum.into_distribution(&speaker))
            .collect();

        WdsizeResult { speakers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{MainTier, Terminator, UtteranceContent, Word};

    fn make_utterance(speaker: &str, words: &[&str]) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    fn file_ctx() -> (talkbank_model::ChatFile, FileContext<'static>) {
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: unsafe { &*(&chat_file as *const _) },
            filename: "test",
            line_map: None,
        };
        (chat_file, ctx)
    }

    #[test]
    fn main_tier_word_sizes() {
        let cmd = WdsizeCommand::new(WdsizeConfig {
            use_main_tier: true,
        });
        let mut state = WdsizeState::default();
        let (_cf, ctx) = file_ctx();

        // "I" = 1, "want" = 4, "cookie" = 6
        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.speakers.len(), 1);
        let sp = &result.speakers[0];
        assert_eq!(sp.total_words, 3);
        assert_eq!(sp.distribution[&1], 1);
        assert_eq!(sp.distribution[&4], 1);
        assert_eq!(sp.distribution[&6], 1);
        assert!((sp.mean() - 3.667).abs() < 0.01);
    }

    #[test]
    fn empty_state_produces_empty_result() {
        let cmd = WdsizeCommand::default();
        let state = WdsizeState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }
}
