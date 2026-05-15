//! MLT — Mean Length of Turn.
//!
//! Calculates mean length of turn in utterances and words. A "turn" is a
//! maximal consecutive sequence of utterances by the same speaker; the
//! turn boundary is detected when a different speaker produces the next
//! utterance.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409101)
//! for the original MLT command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command              | Rust equivalent                        |
//! |---------------------------|----------------------------------------|
//! | `mlt file.cha`            | `chatter analyze mlt file.cha`         |
//! | `mlt +t*CHI file.cha`     | `chatter analyze mlt file.cha -s CHI`  |
//!
//! # Output
//!
//! Per speaker:
//! - Number of turns
//! - Total utterances and words
//! - Mean turn length in utterances (MLT-u) and words (MLT-w)
//! - Sample standard deviation of words per turn
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Turn detection operates on parsed speaker codes from the AST rather
//!   than raw text line prefixes.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::fmt;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{SpeakerCode, Utterance};

use crate::framework::word_filter::{countable_words, has_countable_words};
use crate::framework::{
    AnalysisCommand, CommandOutput, FileContext, TurnCount, UtteranceCount, WordCount,
};

/// Configuration for the MLT command.
#[derive(Debug, Clone, Default)]
pub struct MltConfig {}

/// A single completed turn's statistics.
#[derive(Debug, Default)]
struct Turn {
    /// Number of utterances in this turn.
    utterances: UtteranceCount,
    /// Number of countable words in this turn.
    words: WordCount,
}

/// Per-speaker turn data accumulated during processing.
///
/// Uses a `Vec<Turn>` for completed turns and a single `Turn` for the
/// in-progress turn, replacing the previous parallel `Vec<u64>` fields
/// that represented the same data redundantly.
#[derive(Debug, Default)]
struct SpeakerTurns {
    /// Completed turns (closed when speaker changed or file ended).
    completed: Vec<Turn>,
    /// Current (in-progress) turn.
    current: Turn,
}

impl SpeakerTurns {
    /// Close the current turn (if non-empty) and start a new one.
    ///
    /// # Postcondition
    /// After calling, `current` is reset to a default (empty) turn.
    fn close_turn(&mut self) {
        if self.current.utterances > 0 {
            self.completed.push(Turn {
                utterances: self.current.utterances,
                words: self.current.words,
            });
            self.current = Turn::default();
        }
    }
}

/// Accumulated state for MLT across all files.
#[derive(Debug, Default)]
pub struct MltState {
    /// Per-speaker turn data, keyed by speaker code
    by_speaker: IndexMap<SpeakerCode, SpeakerTurns>,
    /// Speaker code of the most recent utterance (for turn boundary detection)
    last_speaker: Option<SpeakerCode>,
    /// Per-speaker per-utterance word counts (for SD computation)
    words_per_utterance: IndexMap<SpeakerCode, Vec<WordCount>>,
}

/// Typed output from the MLT command.
///
/// Contains per-speaker turn statistics with strongly-typed numeric fields.
#[derive(Debug, Clone, Serialize)]
pub struct MltResult {
    /// Per-speaker MLT statistics, in encounter order.
    pub speakers: Vec<MltSpeakerResult>,
}

/// MLT statistics for a single speaker.
#[derive(Debug, Clone, Serialize)]
pub struct MltSpeakerResult {
    /// Speaker code (e.g., "CHI", "MOT")
    pub speaker: String,
    /// Number of turns
    pub turns: TurnCount,
    /// Total utterances across all turns
    pub utterances: UtteranceCount,
    /// Total words across all turns
    pub words: WordCount,
    /// Mean words per turn (words / turns)
    pub mlt_words: f64,
    /// Mean utterances per turn (utterances / turns)
    pub mlt_utterances: f64,
    /// Mean words per utterance (words / utterances)
    pub words_per_utterance: f64,
    /// Population standard deviation of words-per-utterance (NaN when utterances <= 1)
    pub sd: f64,
}

impl CommandOutput for MltResult {
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
                     \x20 Turns: {}\n\
                     \x20 Total utterances: {}\n\
                     \x20 Total words: {}\n\
                     \x20 MLT (utterances): {:.3}\n\
                     \x20 MLT (words): {:.3}\n",
                    s.speaker, s.turns, s.utterances, s.words, s.mlt_utterances, s.mlt_words
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
    /// MLT for Speaker: *CHI:
    ///   MLT (xxx, yyy and www are EXCLUDED from the word counts, but are INCLUDED in utterance counts):
    ///     Number of: utterances = 2, turns = 2, words = 3
    /// \tRatio of words over turns = 1.500
    /// \tRatio of utterances over turns = 1.000
    /// \tRatio of words over utterances = 1.500
    /// \tStandard deviation = 0.500
    /// ```
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for (i, s) in self.speakers.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            let sd_display = if s.sd.is_nan() {
                "NA".to_owned()
            } else {
                format!("{:.3}", s.sd)
            };
            fmt::write(
                &mut out,
                format_args!(
                    "MLT for Speaker: *{}:\n\
                     \x20 MLT (xxx, yyy and www are EXCLUDED from the word counts, but are INCLUDED in utterance counts):\n\
                     \x20   Number of: utterances = {}, turns = {}, words = {}\n\
                     \tRatio of words over turns = {:.3}\n\
                     \tRatio of utterances over turns = {:.3}\n\
                     \tRatio of words over utterances = {:.3}\n\
                     \tStandard deviation = {}\n",
                    s.speaker,
                    s.utterances,
                    s.turns,
                    s.words,
                    s.mlt_words,
                    s.mlt_utterances,
                    s.words_per_utterance,
                    sd_display,
                ),
            )
            .ok();
        }
        out
    }
}

/// MLT command implementation.
///
/// Tracks turn boundaries by detecting when the speaker changes between
/// consecutive utterances. Each turn accumulates utterance and word counts.
#[derive(Debug, Clone, Default)]
pub struct MltCommand;

impl AnalysisCommand for MltCommand {
    type Config = MltConfig;
    type State = MltState;
    type Output = MltResult;

    /// Update turn state for one lexical utterance and detect speaker boundaries.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Skip utterances with no countable lexical content
        if !has_countable_words(&utterance.main.content.content) {
            return;
        }

        // Arc<str> clone — cheap atomic ref-count increment, no allocation
        let speaker = utterance.main.speaker.clone();

        // Detect turn boundary: if the speaker changed, close all open turns
        if state.last_speaker.as_ref() != Some(&speaker) {
            // Close the previous speaker's turn
            if let Some(ref prev) = state.last_speaker
                && let Some(prev_turns) = state.by_speaker.get_mut(prev)
            {
                prev_turns.close_turn();
            }
            state.last_speaker = Some(speaker.clone());
        }

        let speaker_turns = state
            .by_speaker
            .entry(speaker.clone())
            .or_insert_with(SpeakerTurns::default);

        // Count words using the shared countable_words() iterator
        let word_count = countable_words(&utterance.main.content.content).count() as u64;

        speaker_turns.current.utterances += 1;
        speaker_turns.current.words += word_count;

        // Track per-utterance word count for SD computation
        state
            .words_per_utterance
            .entry(speaker.clone())
            .or_default()
            .push(word_count);
    }

    /// Close any open turns at file boundary so stats do not leak across files.
    fn end_file(&self, _file_context: &FileContext<'_>, state: &mut Self::State) {
        // Close all open turns at file boundary
        for turns in state.by_speaker.values_mut() {
            turns.close_turn();
        }
        state.last_speaker = None;
    }

    /// Compute per-speaker MLT metrics from completed turn sequences.
    fn finalize(&self, state: Self::State) -> MltResult {
        let mut speakers = Vec::new();

        for (speaker, turns) in &state.by_speaker {
            let num_turns = turns.completed.len() as u64;
            if num_turns == 0 {
                continue;
            }

            let total_utterances: u64 = turns.completed.iter().map(|t| t.utterances).sum();
            let total_words: u64 = turns.completed.iter().map(|t| t.words).sum();

            let mlt_utterances = total_utterances as f64 / num_turns as f64;
            let mlt_words = total_words as f64 / num_turns as f64;
            let words_per_utterance = if total_utterances > 0 {
                total_words as f64 / total_utterances as f64
            } else {
                0.0
            };

            // Population standard deviation of words-per-UTTERANCE (not per-turn).
            // CLAN computes SD over individual utterance word counts, using population SD (/ n).
            // When n <= 1, CLAN outputs "NA".
            let utt_words = state.words_per_utterance.get(speaker);
            let num_utts = utt_words.map_or(0, |v| v.len());
            let sd = if num_utts <= 1 {
                f64::NAN
            } else {
                // num_utts > 1 implies `utt_words.is_some()` because
                // `num_utts = utt_words.map_or(0, |v| v.len())`.
                #[allow(clippy::unwrap_used)]
                let utt_words = utt_words.unwrap();
                let mean = words_per_utterance;
                let sum_sq: f64 = utt_words
                    .iter()
                    .map(|&w| {
                        let diff = w as f64 - mean;
                        diff * diff
                    })
                    .sum();
                (sum_sq / num_utts as f64).sqrt()
            };

            speakers.push(MltSpeakerResult {
                speaker: speaker.as_str().to_owned(),
                turns: num_turns,
                utterances: total_utterances,
                words: total_words,
                mlt_words,
                mlt_utterances,
                words_per_utterance,
                sd,
            });
        }

        MltResult { speakers }
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

    /// Consecutive same-speaker utterances should collapse into one turn.
    #[test]
    fn mlt_single_speaker_single_turn() {
        let command = MltCommand;
        let mut state = MltState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // Three consecutive utterances by CHI = one turn
        let u1 = make_utterance("CHI", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["me", "too"]);
        let u3 = make_utterance("CHI", &["please"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);
        command.process_utterance(&u3, &file_ctx, &mut state);
        command.end_file(&file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 1);

        let chi = &result.speakers[0];
        assert_eq!(chi.turns, 1);
        assert_eq!(chi.utterances, 3);
        assert_eq!(chi.words, 6); // 3 + 2 + 1
        assert!((chi.mlt_utterances - 3.0).abs() < 1e-10);
        assert!((chi.mlt_words - 6.0).abs() < 1e-10);
    }

    /// Speaker switches should close the previous turn and start a new one.
    #[test]
    fn mlt_turn_boundaries() {
        let command = MltCommand;
        let mut state = MltState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // CHI → MOT → CHI creates 3 turns (CHI has 2 turns, MOT has 1)
        let u1 = make_utterance("CHI", &["I", "want"]);
        let u2 = make_utterance("CHI", &["more"]);
        let u3 = make_utterance("MOT", &["here", "you", "go"]);
        let u4 = make_utterance("CHI", &["thanks"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);
        command.process_utterance(&u3, &file_ctx, &mut state);
        command.process_utterance(&u4, &file_ctx, &mut state);
        command.end_file(&file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 2);

        // CHI: 2 turns (2 utterances/3 words, then 1 utterance/1 word)
        let chi = &result.speakers[0];
        assert_eq!(chi.speaker, "CHI");
        assert_eq!(chi.turns, 2);
        assert_eq!(chi.utterances, 3);
        assert_eq!(chi.words, 4); // 2 + 1 + 1
        assert!((chi.mlt_utterances - 1.5).abs() < 1e-10);
        assert!((chi.mlt_words - 2.0).abs() < 1e-10);

        // MOT: 1 turn (1 utterance/3 words)
        let mot = &result.speakers[1];
        assert_eq!(mot.speaker, "MOT");
        assert_eq!(mot.turns, 1);
        assert_eq!(mot.utterances, 1);
        assert_eq!(mot.words, 3);
    }

    /// Finalizing untouched state should return no speaker rows.
    #[test]
    fn mlt_empty_state() {
        let command = MltCommand;
        let state = MltState::default();

        let result = command.finalize(state);
        assert_eq!(result.speakers.len(), 0);
    }

    /// Text rendering should include core MLT summary values.
    #[test]
    fn mlt_render_text_format() {
        let result = MltResult {
            speakers: vec![MltSpeakerResult {
                speaker: "CHI".to_owned(),
                turns: 2,
                utterances: 3,
                words: 6,
                mlt_words: 3.0,
                mlt_utterances: 1.5,
                words_per_utterance: 2.0,
                sd: 1.0,
            }],
        };

        let text = result.render_text();
        assert!(text.contains("Speaker: CHI"));
        assert!(text.contains("Turns: 2"));
        assert!(text.contains("MLT (words): 3.000"));
    }

    /// CLAN rendering should preserve legacy labels and ratios.
    #[test]
    fn mlt_render_clan_format() {
        let result = MltResult {
            speakers: vec![MltSpeakerResult {
                speaker: "CHI".to_owned(),
                turns: 2,
                utterances: 2,
                words: 3,
                mlt_words: 1.5,
                mlt_utterances: 1.0,
                words_per_utterance: 1.5,
                sd: 0.5,
            }],
        };

        let clan = result.render_clan();
        assert!(clan.contains("MLT for Speaker: *CHI:"));
        assert!(clan.contains("utterances = 2, turns = 2, words = 3"));
        assert!(clan.contains("Ratio of words over turns = 1.500"));
        assert!(clan.contains("Standard deviation = 0.500"));
    }
}
