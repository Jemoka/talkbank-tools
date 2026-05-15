//! FREQ — Word frequency analysis.
//!
//! Reimplements CLAN's FREQ command, which counts word tokens and types
//! on the main tier and/or `%mor` tier, computing type-token ratio (TTR).
//! FREQ is the most commonly used CLAN command and serves as the foundation
//! for lexical diversity analysis in child language research.
//!
//! Word normalization uses [`NormalizedWord`], which lowercases and strips
//! compound markers (`+`) for grouping, while preserving the original
//! CLAN display form (with `+`) for output.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409093)
//! for the original FREQ command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command              | Rust equivalent                        |
//! |---------------------------|----------------------------------------|
//! | `freq file.cha`           | `chatter analyze freq file.cha`        |
//! | `freq +t*CHI file.cha`    | `chatter analyze freq file.cha -s CHI` |
//!
//! # Output
//!
//! Per-speaker frequency tables with:
//! - Word frequency counts (sorted by count descending, then alphabetically)
//! - Total types (unique words) and tokens (total words)
//! - TTR (type-token ratio = types / tokens)
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::HashMap;
use std::fmt;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{SpeakerCode, Utterance};

use crate::framework::word_filter::countable_words;
use crate::framework::{
    AnalysisCommand, CommandOutput, FileContext, NormalizedWord, TypeCount, WordCount,
    clan_display_form_preserve_case,
};

/// Configuration for the FREQ command.
#[derive(Debug, Clone, Default)]
pub struct FreqConfig {
    /// Count morphemes from %mor tier instead of words from main tier
    pub use_mor: bool,
}

/// Per-speaker frequency data accumulated during processing.
#[derive(Debug, Default)]
struct SpeakerFreq {
    /// Normalized word → count mapping
    counts: HashMap<NormalizedWord, WordCount>,
    /// Normalized word → CLAN display form (preserves `+` in compounds)
    display_forms: HashMap<NormalizedWord, String>,
    /// Total tokens (sum of all counts)
    total_tokens: WordCount,
}

/// Accumulated state for FREQ across all files.
#[derive(Debug, Default)]
pub struct FreqState {
    /// Per-speaker frequency data, keyed by speaker code
    by_speaker: IndexMap<SpeakerCode, SpeakerFreq>,
}

/// Typed output from the FREQ command.
///
/// Contains per-speaker frequency tables with strongly-typed fields.
#[derive(Debug, Clone, Serialize)]
pub struct FreqResult {
    /// Per-speaker frequency data, in encounter order.
    pub speakers: Vec<FreqSpeakerResult>,
}

/// Frequency statistics for a single speaker.
#[derive(Debug, Clone, Serialize)]
pub struct FreqSpeakerResult {
    /// Speaker code (e.g., "CHI", "MOT")
    pub speaker: String,
    /// Word frequency entries, sorted by count descending then alphabetically.
    pub entries: Vec<FreqEntry>,
    /// Number of unique word types
    pub total_types: TypeCount,
    /// Total word tokens
    pub total_tokens: WordCount,
    /// Type-token ratio (types / tokens)
    pub ttr: f64,
}

/// A single word frequency entry.
#[derive(Debug, Clone, Serialize)]
pub struct FreqEntry {
    /// The word (lowercased, cleaned — `+` stripped from compounds)
    pub word: String,
    /// CLAN display form (lowercased but `+` preserved in compounds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_form: Option<String>,
    /// Number of occurrences
    pub count: WordCount,
}

impl CommandOutput for FreqResult {
    /// Our clean text format with aligned table columns.
    fn render_text(&self) -> String {
        let mut out = String::new();
        for (i, s) in self.speakers.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            fmt::write(
                &mut out,
                format_args!(
                    "Speaker: {}\n\
                     \x20 Total types: {}\n\
                     \x20 Total tokens: {}\n\
                     \x20 TTR: {:.3}\n",
                    s.speaker, s.total_types, s.total_tokens, s.ttr
                ),
            )
            .ok();

            // Table with aligned columns
            if !s.entries.is_empty() {
                let count_width = s
                    .entries
                    .iter()
                    .map(|e| e.count.to_string().len())
                    .max()
                    .unwrap_or(5)
                    .max(5); // "Count" header
                let word_width = s
                    .entries
                    .iter()
                    .map(|e| e.word.len())
                    .max()
                    .unwrap_or(4)
                    .max(4); // "Word" header

                fmt::write(
                    &mut out,
                    format_args!(
                        "  {:<cw$}  {:<ww$}\n  {:-<cw$}  {:-<ww$}\n",
                        "Count",
                        "Word",
                        "",
                        "",
                        cw = count_width,
                        ww = word_width
                    ),
                )
                .ok();

                for entry in &s.entries {
                    fmt::write(
                        &mut out,
                        format_args!(
                            "  {:<cw$}  {:<ww$}\n",
                            entry.count,
                            entry.word,
                            cw = count_width,
                            ww = word_width
                        ),
                    )
                    .ok();
                }
            }
        }
        out
    }

    /// CLAN-compatible output matching legacy CLAN character-for-character.
    ///
    /// Format (from CLAN snapshot):
    /// ```text
    /// Speaker: *CHI:
    ///   1 cookie
    ///   1 more
    /// ------------------------------
    ///     3  Total number of different item types used
    ///     3  Total number of items (tokens)
    /// 1.000  Type/Token ratio
    ///     This TTR number was not calculated on the basis of %mor line forms.
    ///     If you want a TTR based on lemmas, run FREQ on the %mor line
    ///     with option: +sm;*,o%
    /// ```
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for (i, s) in self.speakers.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            fmt::write(&mut out, format_args!("Speaker: *{}:\n", s.speaker)).ok();

            // CLAN sorts alphabetically by display form
            let mut sorted_entries: Vec<&FreqEntry> = s.entries.iter().collect();
            sorted_entries.sort_by(|a, b| {
                let a_display = a.display_form.as_deref().unwrap_or(&a.word);
                let b_display = b.display_form.as_deref().unwrap_or(&b.word);
                a_display.cmp(b_display)
            });

            // Word list: " <count> <display_form>"
            for entry in &sorted_entries {
                let display = entry.display_form.as_deref().unwrap_or(&entry.word);
                fmt::write(&mut out, format_args!("{:>3} {}\n", entry.count, display)).ok();
            }

            // Separator and summary
            fmt::write(
                &mut out,
                format_args!(
                    "------------------------------\n\
                     {:>5}  Total number of different item types used\n\
                     {:>5}  Total number of items (tokens)\n\
                     {:.3}  Type/Token ratio\n",
                    s.total_types, s.total_tokens, s.ttr
                ),
            )
            .ok();

            // TTR note (CLAN always includes this for main-tier freq)
            out.push_str(
                "    This TTR number was not calculated on the basis of %mor line forms.\n\
                 \x20   If you want a TTR based on lemmas, run FREQ on the %mor line\n\
                 \x20   with option: +sm;*,o%\n",
            );
        }
        out
    }

    /// CSV rendering with header row.
    fn render_csv(&self) -> String {
        let mut out = String::new();
        for s in &self.speakers {
            out.push_str(&format!("Speaker,{}\n", s.speaker));
            out.push_str("Count,Word\n");
            for entry in &s.entries {
                out.push_str(&format!("{},{}\n", entry.count, entry.word));
            }
            out.push_str(&format!(
                "Total types,{}\nTotal tokens,{}
TTR,{:.3}\n",
                s.total_types, s.total_tokens, s.ttr
            ));
        }
        out
    }
}

/// FREQ command implementation.
///
/// Counts word frequencies on the main tier, producing per-speaker
/// frequency tables with TTR.
#[derive(Debug, Clone, Default)]
pub struct FreqCommand {
    config: FreqConfig,
}

impl FreqCommand {
    /// Create a FREQ command with the given configuration.
    pub fn new(config: FreqConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for FreqCommand {
    type Config = FreqConfig;
    type State = FreqState;
    type Output = FreqResult;

    /// Accumulate per-speaker token counts from main-tier words or `%mor` items.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Arc<str> clone — cheap atomic ref-count increment, no allocation
        let speaker = utterance.main.speaker.clone();
        let speaker_freq = state
            .by_speaker
            .entry(speaker)
            .or_insert_with(SpeakerFreq::default);

        if self.config.use_mor {
            // Count morphemes from %mor tier, using CHAT representation as key.
            // Each MorWord (main + post-clitics) counts as a separate frequency
            // item, matching CLAN's space-separated token counting on %mor.
            if let Some(mor_tier) = utterance.mor_tier() {
                for mor_item in mor_tier.items().iter() {
                    let mut key = String::new();
                    let _ = mor_item.main.write_chat(&mut key);
                    let key = key.to_lowercase();
                    *speaker_freq.counts.entry(NormalizedWord(key)).or_insert(0) += 1;
                    speaker_freq.total_tokens += 1;

                    // Post-clitics are separate frequency items in CLAN
                    for clitic in &mor_item.post_clitics {
                        let mut ckey = String::new();
                        let _ = clitic.write_chat(&mut ckey);
                        let ckey = ckey.to_lowercase();
                        *speaker_freq.counts.entry(NormalizedWord(ckey)).or_insert(0) += 1;
                        speaker_freq.total_tokens += 1;
                    }
                }
            }
        } else {
            // Count words from main tier using the shared countable_words() iterator
            for word in countable_words(&utterance.main.content.content) {
                let key = NormalizedWord::from_word(word);
                speaker_freq
                    .display_forms
                    .entry(key.clone())
                    .or_insert_with(|| clan_display_form_preserve_case(word));
                *speaker_freq.counts.entry(key).or_insert(0) += 1;
                speaker_freq.total_tokens += 1;
            }
        }
    }

    /// Convert accumulated counts into sorted per-speaker frequency tables.
    fn finalize(&self, state: Self::State) -> FreqResult {
        let mut speakers = Vec::new();

        for (speaker, freq) in &state.by_speaker {
            // Sort by count descending, then alphabetically
            let mut raw_entries: Vec<(&NormalizedWord, &u64)> = freq.counts.iter().collect();
            raw_entries.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

            let total_types = freq.counts.len() as u64;
            let total_tokens = freq.total_tokens;
            let ttr = if total_tokens > 0 {
                total_types as f64 / total_tokens as f64
            } else {
                0.0
            };

            let entries: Vec<FreqEntry> = raw_entries
                .iter()
                .map(|(word, count)| {
                    let display = freq.display_forms.get(*word).cloned();
                    FreqEntry {
                        word: word.as_str().to_owned(),
                        display_form: display,
                        count: **count,
                    }
                })
                .collect();

            speakers.push(FreqSpeakerResult {
                speaker: speaker.as_str().to_owned(),
                entries,
                total_types,
                total_tokens,
                ttr,
            });
        }

        FreqResult { speakers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{MainTier, Terminator, UtteranceContent, Word};

    /// Build a minimal utterance with plain words for command tests.
    fn make_utterance(speaker: &str, words: &[&str]) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    /// Counts should remain isolated per speaker key.
    #[test]
    fn freq_counts_words_per_speaker() {
        let command = FreqCommand::default();
        let mut state = FreqState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utterance("CHI", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["I", "want", "more"]);
        let u3 = make_utterance("MOT", &["here", "you", "go"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);
        command.process_utterance(&u3, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 2);

        // CHI section
        let chi = &result.speakers[0];
        assert_eq!(chi.speaker, "CHI");
        assert_eq!(chi.total_tokens, 6);
        assert_eq!(chi.total_types, 4); // i, want, cookie, more

        // MOT section
        let mot = &result.speakers[1];
        assert_eq!(mot.speaker, "MOT");
        assert_eq!(mot.total_tokens, 3);
        assert_eq!(mot.total_types, 3);
    }

    /// TTR should be computed as `types / tokens`.
    #[test]
    fn freq_ttr_calculation() {
        let command = FreqCommand::default();
        let mut state = FreqState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // All same word → TTR = 1/5 = 0.200
        let u = make_utterance("CHI", &["the", "the", "the", "the", "the"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        let result = command.finalize(state);
        let chi = &result.speakers[0];
        assert!((chi.ttr - 0.2).abs() < 1e-10);
    }

    /// Text rendering should include speaker summary and token rows.
    #[test]
    fn freq_render_text_format() {
        let result = FreqResult {
            speakers: vec![FreqSpeakerResult {
                speaker: "CHI".to_owned(),
                entries: vec![
                    FreqEntry {
                        word: "want".to_owned(),
                        display_form: None,
                        count: 2,
                    },
                    FreqEntry {
                        word: "cookie".to_owned(),
                        display_form: None,
                        count: 1,
                    },
                ],
                total_types: 2,
                total_tokens: 3,
                ttr: 0.667,
            }],
        };

        let text = result.render_text();
        assert!(text.contains("Speaker: CHI"));
        assert!(text.contains("Total types: 2"));
        assert!(text.contains("want"));
        assert!(text.contains("cookie"));
    }

    /// CLAN rendering should expose legacy-style summary labels.
    #[test]
    fn freq_render_clan_format() {
        let result = FreqResult {
            speakers: vec![FreqSpeakerResult {
                speaker: "CHI".to_owned(),
                entries: vec![
                    FreqEntry {
                        word: "want".to_owned(),
                        display_form: None,
                        count: 2,
                    },
                    FreqEntry {
                        word: "cookie".to_owned(),
                        display_form: None,
                        count: 1,
                    },
                ],
                total_types: 2,
                total_tokens: 3,
                ttr: 0.667,
            }],
        };

        let clan = result.render_clan();
        assert!(clan.contains("Speaker: *CHI:"));
        assert!(clan.contains("2 want"));
        assert!(clan.contains("1 cookie"));
        assert!(clan.contains("Total number of different item types used"));
        assert!(clan.contains("Total number of items (tokens)"));
        assert!(clan.contains("Type/Token ratio"));
    }
}
