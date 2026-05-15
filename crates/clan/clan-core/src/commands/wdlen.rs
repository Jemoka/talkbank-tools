//! WDLEN -- Word Length Distribution (6-section CLAN format).
//!
//! Computes six distribution tables matching CLAN's output:
//! 1. Word lengths in characters
//! 2. Utterance lengths in words
//! 3. Turn lengths in utterances
//! 4. Turn lengths in words
//! 5. Word lengths in morphemes (requires %mor)
//! 6. Utterance lengths in morphemes (requires %mor)
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409247)
//! for the original WDLEN command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command               | Rust equivalent                         |
//! |----------------------------|-----------------------------------------|
//! | `wdlen file.cha`           | `chatter analyze wdlen file.cha`        |
//! | `wdlen +t*CHI file.cha`    | `chatter analyze wdlen file.cha -s CHI` |
//!
//! # Differences from CLAN
//!
//! - **Brown's morpheme rules**: Section 5 = stem + Brown's suffix (no POS).
//!   Section 6 = POS + stem + Brown's suffix. Brown's suffixes are the same 7
//!   strings as MLU: `PL`, `PAST`, `Past`, `POSS`, `PASTP`, `Pastp`, `PRESP`.
//! - **Clitic handling**: Section 5 merges main+clitics as one word. Section 6
//!   counts POS only for main word.
//! - **Apostrophe stripping**: Characters counted after removing apostrophes,
//!   matching CLAN.
//! - **Reverse speaker order**: CLAN's linked-list prepend pattern replicated.
//! - **XML footer**: `</Table></Worksheet></Workbook>` appended to match CLAN.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).

use std::collections::BTreeMap;
use std::fmt::Write;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{SpeakerCode, Utterance};

use crate::framework::word_filter::{countable_words, has_countable_words};
use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
};

/// Configuration for the WDLEN command.
#[derive(Debug, Clone, Default)]
pub struct WdlenConfig {}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A single speaker's distribution for one section.
#[derive(Debug, Clone, Serialize)]
pub struct WdlenDistribution {
    /// Speaker code (e.g. "MOT", "CHI").
    pub speaker: String,
    /// Value -> count mapping (sorted by value for deterministic output).
    pub distribution: BTreeMap<usize, u64>,
    /// Total number of items (for mean denominator).
    pub total_items: u64,
    /// Sum of all values (for mean numerator).
    pub total_value: u64,
}

impl WdlenDistribution {
    fn mean(&self) -> f64 {
        if self.total_items == 0 {
            0.0
        } else {
            self.total_value as f64 / self.total_items as f64
        }
    }
}

/// Typed output for the WDLEN command -- 6 distribution sections.
#[derive(Debug, Clone, Serialize)]
pub struct WdlenResult {
    /// Section 1: word lengths in characters.
    pub word_lengths: Vec<WdlenDistribution>,
    /// Section 2: utterance lengths in words.
    pub utt_word_lengths: Vec<WdlenDistribution>,
    /// Section 3: turn lengths in utterances.
    pub turn_utt_lengths: Vec<WdlenDistribution>,
    /// Section 4: turn lengths in words.
    pub turn_word_lengths: Vec<WdlenDistribution>,
    /// Section 5: word lengths in morphemes.
    pub morph_lengths: Vec<WdlenDistribution>,
    /// Section 6: utterance lengths in morphemes.
    pub utt_morph_lengths: Vec<WdlenDistribution>,
}

/// Render one distribution section in CLAN table format.
///
/// CLAN uses fixed 5-char right-justified columns for all values.
/// The label field width is `max("lengths".len(), max("*SPK:".len())) + 1`.
fn render_section(out: &mut String, title: &str, distributions: &[WdlenDistribution]) {
    let _ = writeln!(out, "{title}");

    // Find the max length value across all speakers in this section.
    let max_len = distributions
        .iter()
        .flat_map(|d| d.distribution.keys())
        .copied()
        .max()
        .unwrap_or(1);

    // CLAN uses fixed 5-char columns.
    let col_width = 5;

    // Label field: max of "lengths" and longest "*SPK:" plus 1 for padding.
    let max_speaker_label = distributions
        .iter()
        .map(|d| d.speaker.len() + 2) // "*" + speaker + ":"
        .max()
        .unwrap_or(0);
    let label_width = "lengths".len().max(max_speaker_label) + 1;

    let mut header = format!("{:<label_width$}", "lengths");
    for col in 1..=max_len {
        let _ = write!(header, "{:>col_width$}", col);
    }
    let _ = write!(header, "{:>7}", "Mean");
    let _ = writeln!(out, "{header}");

    // Per-speaker rows (CLAN outputs in reverse encounter order).
    for dist in distributions {
        let speaker_label = format!("*{}:", dist.speaker);
        let mut row = format!("{:<label_width$}", speaker_label);
        for col in 1..=max_len {
            let count = dist.distribution.get(&col).copied().unwrap_or(0);
            let _ = write!(row, "{:>col_width$}", count);
        }
        let _ = write!(row, "{:>7.3}", dist.mean());
        let _ = writeln!(out, "{row}");
    }
}

impl WdlenResult {
    /// Convert to the shared section/table model for text rendering.
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("wdlen");
        for data in &self.word_lengths {
            let mut fields = IndexMap::new();
            fields.insert("Total words".to_owned(), data.total_items.to_string());
            fields.insert("Mean word length".to_owned(), format!("{:.3}", data.mean()));

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
            section.fields = fields;
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for WdlenResult {
    /// Render via the shared tabular text formatter (simplified view).
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render the full 6-section CLAN format.
    ///
    /// CLAN outputs speakers in reverse encounter order (its linked-list
    /// iteration pattern). We reverse each section's distributions to match.
    fn render_clan(&self) -> String {
        let sections: &[(&str, &[WdlenDistribution])] = &[
            (
                "Number of words of each length in characters",
                &self.word_lengths,
            ),
            (
                "Number of utterances of each of these lengths in words",
                &self.utt_word_lengths,
            ),
            (
                "Number of single turns of each of these lengths in utterances",
                &self.turn_utt_lengths,
            ),
            (
                "Number of single turns of each of these lengths in words",
                &self.turn_word_lengths,
            ),
            (
                "Number of words of each of these morpheme lengths",
                &self.morph_lengths,
            ),
            (
                "Number of utterances of each of these lengths in morphemes",
                &self.utt_morph_lengths,
            ),
        ];

        let mut out = String::new();
        for (i, (title, dists)) in sections.iter().enumerate() {
            if i > 0 {
                let _ = writeln!(out, "-------");
            }
            let _ = writeln!(out);
            // Reverse to match CLAN's reverse-encounter-order iteration.
            let reversed: Vec<_> = dists.iter().rev().cloned().collect();
            render_section(&mut out, title, &reversed);
        }

        // CLAN appends XML closing tags at the end.
        let _ = writeln!(out, "  </Table>");
        let _ = writeln!(out, " </Worksheet>");
        let _ = write!(out, "</Workbook>");

        out
    }
}

// ---------------------------------------------------------------------------
// Accumulation state
// ---------------------------------------------------------------------------

/// Per-speaker accumulator for a single distribution dimension.
#[derive(Debug, Default)]
struct DistAccum {
    distribution: BTreeMap<usize, u64>,
    total_items: u64,
    total_value: u64,
}

impl DistAccum {
    fn record(&mut self, value: usize) {
        *self.distribution.entry(value).or_insert(0) += 1;
        self.total_items += 1;
        self.total_value += value as u64;
    }

    fn into_distribution(self, speaker: &str) -> WdlenDistribution {
        WdlenDistribution {
            speaker: speaker.to_owned(),
            distribution: self.distribution,
            total_items: self.total_items,
            total_value: self.total_value,
        }
    }
}

/// Per-speaker data tracking the current turn.
#[derive(Debug, Default)]
struct SpeakerAccum {
    /// Section 1: word char lengths.
    word_lengths: DistAccum,
    /// Section 2: utterance word counts.
    utt_word_counts: DistAccum,
    /// Section 5: per-word morpheme lengths.
    morph_lengths: DistAccum,
    /// Section 6: utterance morpheme counts.
    utt_morph_counts: DistAccum,
    /// Current turn: utterance count.
    current_turn_utts: u64,
    /// Current turn: word count.
    current_turn_words: u64,
    /// Turn utterance counts (section 3).
    turn_utt_counts: DistAccum,
    /// Turn word counts (section 4).
    turn_word_counts: DistAccum,
}

impl SpeakerAccum {
    /// Close the current turn and record its stats.
    fn close_turn(&mut self) {
        if self.current_turn_utts > 0 {
            self.turn_utt_counts.record(self.current_turn_utts as usize);
            self.turn_word_counts
                .record(self.current_turn_words as usize);
            self.current_turn_utts = 0;
            self.current_turn_words = 0;
        }
    }
}

/// Accumulated state for WDLEN across all files.
#[derive(Debug, Default)]
pub struct WdlenState {
    by_speaker: IndexMap<SpeakerCode, SpeakerAccum>,
    last_speaker: Option<SpeakerCode>,
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// WDLEN command implementation.
#[derive(Debug, Clone, Default)]
pub struct WdlenCommand;

/// Brown's (1973) counted suffixes — same as MLU.
const COUNTED_SUFFIXES: &[&str] = &["PL", "PAST", "Past", "POSS", "PASTP", "Pastp", "PRESP"];

/// Check if a MorWord has any Brown's counted suffix.
fn has_counted_suffix(word: &talkbank_model::MorWord) -> bool {
    word.features
        .iter()
        .any(|f| COUNTED_SUFFIXES.contains(&f.value()))
}

/// Count morphemes per word for section 5 (Brown's rules: stem + counted suffix).
fn word_morpheme_count(word: &talkbank_model::MorWord) -> u64 {
    1 + if has_counted_suffix(word) { 1 } else { 0 }
}

/// Count morphemes per word for section 6 (POS + stem + counted suffix).
///
/// CLAN includes the POS tag as a morpheme in per-utterance totals.
fn word_morpheme_count_with_pos(word: &talkbank_model::MorWord) -> u64 {
    2 + if has_counted_suffix(word) { 1 } else { 0 }
}

impl AnalysisCommand for WdlenCommand {
    type Config = WdlenConfig;
    type State = WdlenState;
    type Output = WdlenResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        if !has_countable_words(&utterance.main.content.content) {
            return;
        }

        let speaker = utterance.main.speaker.clone();

        // Detect turn boundary: close previous speaker's turn on speaker change.
        if state.last_speaker.as_ref() != Some(&speaker) {
            if let Some(ref prev) = state.last_speaker
                && let Some(prev_data) = state.by_speaker.get_mut(prev)
            {
                prev_data.close_turn();
            }
            state.last_speaker = Some(speaker.clone());
        }

        let data = state
            .by_speaker
            .entry(speaker)
            .or_insert_with(SpeakerAccum::default);

        // Section 1: per-word character lengths (CLAN strips apostrophes).
        let mut word_count: u64 = 0;
        for word in countable_words(&utterance.main.content.content) {
            let char_len = word.cleaned_text().chars().filter(|&c| c != '\'').count();
            data.word_lengths.record(char_len);
            word_count += 1;
        }

        // Section 2: utterance word count.
        data.utt_word_counts.record(word_count as usize);

        // Turn tracking (sections 3 & 4).
        data.current_turn_utts += 1;
        data.current_turn_words += word_count;

        // Sections 5 & 6: morpheme counts (only if %mor tier present).
        // CLAN treats clitic pairs (main~clitic) as single words for section 5.
        // Section 5: per-word = stem + Brown's suffix, clitics merged into one word.
        // Section 6: per-utterance = POS(main only) + stems + Brown's suffixes.
        if let Some(mor_tier) = utterance.mor_tier() {
            let mut utt_morphemes: u64 = 0;
            for mor_item in mor_tier.items().iter() {
                // Section 5: entire Mor item (main + clitics) = one word entry.
                let mut word_morphs = word_morpheme_count(&mor_item.main);
                for clitic in &mor_item.post_clitics {
                    word_morphs += word_morpheme_count(clitic);
                }
                data.morph_lengths.record(word_morphs as usize);

                // Section 6: POS counted only for main word, not clitics.
                utt_morphemes += word_morpheme_count_with_pos(&mor_item.main);
                for clitic in &mor_item.post_clitics {
                    // Clitic: stem + Brown's suffix, no POS.
                    utt_morphemes += word_morpheme_count(clitic);
                }
            }
            data.utt_morph_counts.record(utt_morphemes as usize);
        }
    }

    /// Close open turns at file boundary.
    fn end_file(&self, _file_context: &FileContext<'_>, state: &mut Self::State) {
        for data in state.by_speaker.values_mut() {
            data.close_turn();
        }
        state.last_speaker = None;
    }

    fn finalize(&self, state: Self::State) -> WdlenResult {
        let mut word_lengths = Vec::new();
        let mut utt_word_lengths = Vec::new();
        let mut turn_utt_lengths = Vec::new();
        let mut turn_word_lengths = Vec::new();
        let mut morph_lengths = Vec::new();
        let mut utt_morph_lengths = Vec::new();

        for (speaker, data) in state.by_speaker {
            let name = speaker.as_str();
            word_lengths.push(data.word_lengths.into_distribution(name));
            utt_word_lengths.push(data.utt_word_counts.into_distribution(name));
            turn_utt_lengths.push(data.turn_utt_counts.into_distribution(name));
            turn_word_lengths.push(data.turn_word_counts.into_distribution(name));
            morph_lengths.push(data.morph_lengths.into_distribution(name));
            utt_morph_lengths.push(data.utt_morph_counts.into_distribution(name));
        }

        WdlenResult {
            word_lengths,
            utt_word_lengths,
            turn_utt_lengths,
            turn_word_lengths,
            morph_lengths,
            utt_morph_lengths,
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

    fn file_ctx() -> (talkbank_model::ChatFile, FileContext<'static>) {
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        // SAFETY: we only use this in tests where the lifetime is scoped
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: unsafe { &*(&chat_file as *const _) },
            filename: "test",
            line_map: None,
        };
        (chat_file, ctx)
    }

    #[test]
    fn basic_word_length_distribution() {
        let command = WdlenCommand;
        let mut state = WdlenState::default();
        let (_cf, ctx) = file_ctx();

        // "I" = 1, "want" = 4, "cookie" = 6
        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.word_lengths.len(), 1);

        let chi = &result.word_lengths[0];
        assert_eq!(chi.total_items, 3);
        assert_eq!(format!("{:.3}", chi.mean()), "3.667");
        assert_eq!(chi.distribution[&1], 1);
        assert_eq!(chi.distribution[&4], 1);
        assert_eq!(chi.distribution[&6], 1);
    }

    #[test]
    fn utterance_word_counts() {
        let command = WdlenCommand;
        let mut state = WdlenState::default();
        let (_cf, ctx) = file_ctx();

        let u1 = make_utterance("CHI", &["I", "want"]);
        let u2 = make_utterance("CHI", &["more", "cookie", "please"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let result = command.finalize(state);
        let chi_utt = &result.utt_word_lengths[0];
        // Utterance 1 has 2 words, utterance 2 has 3 words
        assert_eq!(chi_utt.distribution[&2], 1);
        assert_eq!(chi_utt.distribution[&3], 1);
        assert_eq!(chi_utt.total_items, 2);
    }

    #[test]
    fn turn_detection_across_speakers() {
        let command = WdlenCommand;
        let mut state = WdlenState::default();
        let (_cf, ctx) = file_ctx();

        // MOT turn: 2 utterances
        let u1 = make_utterance("MOT", &["look", "here"]);
        let u2 = make_utterance("MOT", &["see"]);
        // CHI turn: 1 utterance
        let u3 = make_utterance("CHI", &["yes"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.process_utterance(&u3, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let result = command.finalize(state);
        // MOT: 1 turn with 2 utterances
        let mot_turn_utts = &result.turn_utt_lengths[0];
        assert_eq!(mot_turn_utts.speaker, "MOT");
        assert_eq!(mot_turn_utts.distribution[&2], 1);

        // CHI: 1 turn with 1 utterance
        let chi_turn_utts = &result.turn_utt_lengths[1];
        assert_eq!(chi_turn_utts.speaker, "CHI");
        assert_eq!(chi_turn_utts.distribution[&1], 1);
    }

    #[test]
    fn empty_state_produces_empty_result() {
        let command = WdlenCommand;
        let state = WdlenState::default();
        let result = command.finalize(state);
        assert!(result.word_lengths.is_empty());
    }

    #[test]
    fn clan_render_format() {
        let command = WdlenCommand;
        let mut state = WdlenState::default();
        let (_cf, ctx) = file_ctx();

        let u1 = make_utterance("CHI", &["I", "want"]);
        let u2 = make_utterance("MOT", &["ok"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let result = command.finalize(state);
        let clan = result.render_clan();
        // Verify it contains expected section titles
        assert!(clan.contains("Number of words of each length in characters"));
        assert!(clan.contains("Number of utterances of each of these lengths in words"));
        assert!(clan.contains("Number of single turns of each of these lengths in utterances"));
        assert!(clan.contains("Number of single turns of each of these lengths in words"));
        assert!(clan.contains("Number of words of each of these morpheme lengths"));
        assert!(clan.contains("Number of utterances of each of these lengths in morphemes"));
        // Verify separator
        assert!(clan.contains("-------"));
        // Verify speaker labels
        assert!(clan.contains("*CHI:"));
        assert!(clan.contains("*MOT:"));
    }
}
