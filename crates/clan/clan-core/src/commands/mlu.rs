//! MLU — Mean Length of Utterance.
//!
//! Calculates mean length of utterance in morphemes from the `%mor` tier.
//! When no `%mor` tier is available and not in `words_only` mode, reports
//! "utterances = 0, morphemes = 0" (matching CLAN behavior — no fallback
//! to word counting).
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409094)
//! for the original MLU command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command              | Rust equivalent                        |
//! |---------------------------|----------------------------------------|
//! | `mlu file.cha`            | `chatter analyze mlu file.cha`         |
//! | `mlu +t*CHI file.cha`     | `chatter analyze mlu file.cha -s CHI`  |
//!
//! # MLU Calculation
//!
//! For each utterance:
//! 1. Count morphemes on the `%mor` tier: 1 per stem + 1 per `-` suffix
//!    (bound morpheme) + 1 per `~` clitic stem + 1 per clitic `-` suffix.
//!    Fusional features (`&`) do NOT count.
//! 2. If no `%mor` tier, skip the utterance (report 0 utterances for the speaker)
//!
//! Per speaker, compute:
//! - Number of utterances
//! - Total morphemes
//! - MLU (mean)
//! - Standard deviation (sample, n-1)
//! - Range (min, max)
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Morpheme counting uses parsed `%mor` tier structure (MorWord features
//!   and post-clitics) rather than text splitting on spaces and delimiters.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::fmt;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{Mor, SpeakerCode, Utterance};

use crate::framework::word_filter::{countable_words_in_utterance, has_countable_words};
use crate::framework::{
    AnalysisCommand, CommandOutput, FileContext, MorphemeCount, UtteranceCount,
};

/// Configuration for the MLU command.
#[derive(Debug, Clone, Default)]
pub struct MluConfig {
    /// Use word count from main tier instead of morpheme count from %mor
    pub words_only: bool,
}

/// Per-speaker MLU data accumulated during processing.
#[derive(Debug, Default)]
struct SpeakerMlu {
    /// Morpheme (or word) counts per utterance
    utterance_lengths: Vec<MorphemeCount>,
}

/// Accumulated state for MLU across all files.
#[derive(Debug, Default)]
pub struct MluState {
    /// Per-speaker MLU data, keyed by speaker code
    by_speaker: IndexMap<SpeakerCode, SpeakerMlu>,
}

/// Typed output from the MLU command.
///
/// Contains per-speaker MLU statistics with strongly-typed numeric fields,
/// replacing the stringly-typed `AnalysisResult` for programmatic access.
#[derive(Debug, Clone, Serialize)]
pub struct MluResult {
    /// Per-speaker MLU statistics, in encounter order.
    pub speakers: Vec<MluSpeakerResult>,
}

/// MLU statistics for a single speaker.
#[derive(Debug, Clone, Serialize)]
pub struct MluSpeakerResult {
    /// Speaker code (e.g., "CHI", "MOT")
    pub speaker: String,
    /// Number of utterances included in the calculation
    pub utterances: UtteranceCount,
    /// Total morphemes (or words, if `--words` mode) across all utterances
    pub morphemes: MorphemeCount,
    /// Mean length of utterance (morphemes / utterances)
    pub mlu: f64,
    /// Population standard deviation of utterance lengths (/ n denominator).
    /// NAN when n=1 (rendered as "NA" in CLAN format).
    pub sd: f64,
    /// Minimum utterance length
    pub min: MorphemeCount,
    /// Maximum utterance length
    pub max: MorphemeCount,
}

impl CommandOutput for MluResult {
    /// Our clean text format.
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
                     \x20 Utterances: {}\n\
                     \x20 Total morphemes: {}\n\
                     \x20 MLU: {:.3}\n\
                     \x20 SD: {:.3}\n\
                     \x20 Range: {}-{}\n",
                    s.speaker, s.utterances, s.morphemes, s.mlu, s.sd, s.min, s.max
                ),
            )
            .ok();
        }
        out
    }

    /// CLAN-compatible output matching legacy CLAN character-for-character.
    ///
    /// Format (from CLAN snapshot):
    /// ```text
    /// MLU for Speaker: *CHI:
    ///   MLU (xxx, yyy and www are EXCLUDED from the utterance and morpheme counts):
    /// \tNumber of: utterances = 2, morphemes = 3
    /// \tRatio of morphemes over utterances = 1.500
    /// \tStandard deviation = 0.500
    /// ```
    ///
    /// When utterances = 0, only header + counts are emitted (no Ratio/SD).
    /// When n = 1, SD is printed as "NA" (sample SD undefined).
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for (i, s) in self.speakers.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            // Header and counts are always present
            fmt::write(
                &mut out,
                format_args!(
                    "MLU for Speaker: *{}:\n\
                     \x20 MLU (xxx, yyy and www are EXCLUDED from the utterance and morpheme counts):\n\
                     \tNumber of: utterances = {}, morphemes = {}\n",
                    s.speaker, s.utterances, s.morphemes
                ),
            )
            .ok();

            // When utterances = 0, CLAN omits Ratio and SD lines entirely
            if s.utterances > 0 {
                fmt::write(
                    &mut out,
                    format_args!("\tRatio of morphemes over utterances = {:.3}\n", s.mlu),
                )
                .ok();

                if s.sd.is_nan() {
                    // n=1: sample SD is undefined
                    fmt::write(&mut out, format_args!("\tStandard deviation = NA\n")).ok();
                } else {
                    fmt::write(
                        &mut out,
                        format_args!("\tStandard deviation = {:.3}\n", s.sd),
                    )
                    .ok();
                }
            }
        }
        out
    }
}

/// MLU command implementation.
///
/// Counts morphemes per utterance from the %mor tier (or words from
/// the main tier), computing mean, SD, and range per speaker.
#[derive(Debug, Clone, Default)]
pub struct MluCommand {
    config: MluConfig,
}

impl MluCommand {
    /// Create an MLU command with the given configuration.
    pub fn new(config: MluConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for MluCommand {
    type Config = MluConfig;
    type State = MluState;
    type Output = MluResult;

    /// Record one utterance length for the current speaker when lexical material exists.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Skip utterances with no countable lexical content (e.g., "xxx .")
        // These would deflate MLU by adding zero-morpheme utterances to the
        // denominator. CLAN achieves this by string-prefix exclusion; we use
        // the AST's semantic word classification instead.
        if !has_countable_words(&utterance.main.content.content) {
            return;
        }

        // Arc<str> clone — cheap atomic ref-count increment, no allocation
        let speaker = utterance.main.speaker.clone();

        let count = if self.config.words_only {
            Some(count_words_in_utterance(utterance))
        } else {
            count_morphemes_in_utterance(utterance)
        };

        // Always register the speaker so they appear in output, even when
        // no %mor tier is available (CLAN parity: shows "utterances = 0").
        let speaker_mlu = state
            .by_speaker
            .entry(speaker)
            .or_insert_with(SpeakerMlu::default);

        // When no %mor tier exists and not in words_only mode, CLAN reports
        // "utterances = 0, morphemes = 0" — it does NOT fall back to counting
        // words. We skip adding the utterance length but still register the
        // speaker above.
        if let Some(count) = count {
            speaker_mlu.utterance_lengths.push(count);
        }
    }

    /// Compute per-speaker MLU aggregates (mean, SD, min, max) from collected lengths.
    fn finalize(&self, state: Self::State) -> MluResult {
        let mut speakers = Vec::new();

        for (speaker, mlu_data) in &state.by_speaker {
            let n = mlu_data.utterance_lengths.len() as u64;

            // Include speakers even when they have 0 utterances (e.g., no %mor
            // tier present). CLAN outputs "utterances = 0, morphemes = 0" for
            // these speakers.
            if n == 0 {
                speakers.push(MluSpeakerResult {
                    speaker: speaker.as_str().to_owned(),
                    utterances: 0,
                    morphemes: 0,
                    mlu: 0.0,
                    sd: 0.0,
                    min: 0,
                    max: 0,
                });
                continue;
            }

            let total: u64 = mlu_data.utterance_lengths.iter().sum();
            let mean = total as f64 / n as f64;

            // Population standard deviation (/ n denominator) to match CLAN.
            // When n=1, SD is undefined (NAN) — rendered as "NA" in CLAN format.
            let sd = if n == 1 {
                f64::NAN
            } else {
                let sum_sq: f64 = mlu_data
                    .utterance_lengths
                    .iter()
                    .map(|&len| {
                        let diff = len as f64 - mean;
                        diff * diff
                    })
                    .sum();
                (sum_sq / n as f64).sqrt()
            };

            let min = mlu_data
                .utterance_lengths
                .iter()
                .copied()
                .min()
                .unwrap_or(0);
            let max = mlu_data
                .utterance_lengths
                .iter()
                .copied()
                .max()
                .unwrap_or(0);

            speakers.push(MluSpeakerResult {
                speaker: speaker.as_str().to_owned(),
                utterances: n,
                morphemes: total,
                mlu: mean,
                sd,
                min,
                max,
            });
        }

        MluResult { speakers }
    }
}

/// Count morphemes in an utterance from the %mor tier.
///
/// CLAN counts bound morphemes separately: each `-` suffix (e.g., `-PL` in
/// `cookie-PL`) and each `~` clitic (e.g., `~aux|be&PRES`) adds an additional
/// morpheme. Fusional features marked with `&` do NOT count.
///
/// For each `Mor` item: 1 (stem) + features.len() (bound morphemes via `-`)
/// + for each post-clitic: 1 + clitic.features.len().
///
/// Returns `None` when no %mor tier is present (CLAN reports "utterances = 0"
/// in this case, rather than falling back to word counting).
fn count_morphemes_in_utterance(utterance: &Utterance) -> Option<u64> {
    let mor_tier = utterance.mor_tier()?;
    let total: u64 = mor_tier.items().iter().map(count_morphemes_in_mor).sum();
    Some(total)
}

/// Brown's (1973) morpheme-counting suffixes: features that represent
/// bound morphemes in English. CLAN counts stem + 1 if ANY feature matches.
/// Both all-uppercase (traditional CLAN) and title-case (UD) are accepted.
const COUNTED_SUFFIXES: &[&str] = &["PL", "PAST", "Past", "POSS", "PASTP", "Pastp", "PRESP"];

/// Check whether a `MorWord` has any feature that counts as a bound morpheme.
fn has_counted_suffix(word: &talkbank_model::MorWord) -> bool {
    word.features
        .iter()
        .any(|f| COUNTED_SUFFIXES.contains(&f.value()))
}

/// Count morphemes contributed by a single `Mor` item.
///
/// CLAN counts 1 for the stem, plus 1 if the word has any Brown morpheme
/// suffix (-PL, -PAST, -POSS, -PASTP, -PRESP). Multiple suffixes still
/// count as just +1 total. Post-clitics follow the same rule.
fn count_morphemes_in_mor(mor: &Mor) -> u64 {
    let main_count = 1 + if has_counted_suffix(&mor.main) { 1 } else { 0 };
    let clitic_count: u64 = mor
        .post_clitics
        .iter()
        .map(|c| 1 + if has_counted_suffix(c) { 1 } else { 0 })
        .sum();
    main_count + clitic_count
}

/// Count words in an utterance from the main tier (fallback when no %mor).
///
/// Uses the shared [`countable_words_in_utterance`] iterator to avoid
/// duplicating the tree-walking logic.
fn count_words_in_utterance(utterance: &Utterance) -> u64 {
    countable_words_in_utterance(utterance).count() as u64
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

    /// Without `%mor`, CLAN reports utterances = 0, morphemes = 0 (no fallback).
    /// The speaker is still registered and appears in output.
    #[test]
    fn mlu_no_mor_reports_zero() {
        let command = MluCommand::default();
        let mut state = MluState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utterance("CHI", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["me", "too"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 1);

        let chi = &result.speakers[0];
        assert_eq!(chi.utterances, 0);
        assert_eq!(chi.morphemes, 0);
        assert!((chi.mlu - 0.0).abs() < 1e-10);
    }

    /// In words_only mode, without %mor, word counting is used as fallback.
    #[test]
    fn mlu_words_only_counts_words() {
        let config = MluConfig { words_only: true };
        let command = MluCommand::new(config);
        let mut state = MluState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // 3 words, 2 words, 4 words → mean = 3.0
        let u1 = make_utterance("CHI", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["me", "too"]);
        let u3 = make_utterance("CHI", &["I", "want", "more", "cookie"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);
        command.process_utterance(&u3, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 1);

        let chi = &result.speakers[0];
        assert_eq!(chi.utterances, 3);
        assert_eq!(chi.morphemes, 9);
        assert!((chi.mlu - 3.0).abs() < 1e-10);
    }

    /// Finalizing empty state should produce no speaker entries.
    #[test]
    fn mlu_handles_empty_speaker() {
        let command = MluCommand::default();
        let state = MluState::default();

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 0);
    }

    /// Utterance lengths should be tracked independently per speaker (words_only mode).
    #[test]
    fn mlu_per_speaker_separation() {
        let config = MluConfig { words_only: true };
        let command = MluCommand::new(config);
        let mut state = MluState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utterance("CHI", &["me", "want"]);
        let u2 = make_utterance("MOT", &["you", "can", "have", "it"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 2);

        assert!((result.speakers[0].mlu - 2.0).abs() < 1e-10);
        assert!((result.speakers[1].mlu - 4.0).abs() < 1e-10);
    }

    /// Text rendering should include key MLU summary values.
    #[test]
    fn mlu_render_text_format() {
        let result = MluResult {
            speakers: vec![MluSpeakerResult {
                speaker: "CHI".to_owned(),
                utterances: 3,
                morphemes: 9,
                mlu: 3.0,
                sd: 0.816,
                min: 2,
                max: 4,
            }],
        };

        let text = result.render_text();
        assert!(text.contains("Speaker: CHI"));
        assert!(text.contains("Utterances: 3"));
        assert!(text.contains("MLU: 3.000"));
    }

    /// CLAN rendering should retain legacy line labels and numeric formatting.
    #[test]
    fn mlu_render_clan_format() {
        let result = MluResult {
            speakers: vec![MluSpeakerResult {
                speaker: "CHI".to_owned(),
                utterances: 2,
                morphemes: 3,
                mlu: 1.5,
                sd: 0.707,
                min: 1,
                max: 2,
            }],
        };

        let clan = result.render_clan();
        assert!(clan.contains("MLU for Speaker: *CHI:"));
        assert!(clan.contains("utterances = 2, morphemes = 3"));
        assert!(clan.contains("Ratio of morphemes over utterances = 1.500"));
        assert!(clan.contains("Standard deviation = 0.707"));
    }

    /// CLAN rendering with 0 utterances should omit Ratio and SD lines.
    #[test]
    fn mlu_render_clan_zero_utterances() {
        let result = MluResult {
            speakers: vec![MluSpeakerResult {
                speaker: "CHI".to_owned(),
                utterances: 0,
                morphemes: 0,
                mlu: 0.0,
                sd: 0.0,
                min: 0,
                max: 0,
            }],
        };

        let clan = result.render_clan();
        assert!(clan.contains("MLU for Speaker: *CHI:"));
        assert!(clan.contains("utterances = 0, morphemes = 0"));
        assert!(!clan.contains("Ratio"));
        assert!(!clan.contains("Standard deviation"));
    }

    /// CLAN rendering with n=1 should show SD as "NA".
    #[test]
    fn mlu_render_clan_single_utterance() {
        let result = MluResult {
            speakers: vec![MluSpeakerResult {
                speaker: "CHI".to_owned(),
                utterances: 1,
                morphemes: 3,
                mlu: 3.0,
                sd: f64::NAN,
                min: 3,
                max: 3,
            }],
        };

        let clan = result.render_clan();
        assert!(clan.contains("Ratio of morphemes over utterances = 3.000"));
        assert!(clan.contains("Standard deviation = NA"));
    }
}
