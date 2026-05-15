//! FLUCALC — Fluency calculation (disfluency metrics).
//!
//! Detects and quantifies disfluencies in speech transcripts, producing
//! per-speaker counts of stuttering-like disfluencies (SLD) and typical
//! disfluencies (TD). FLUCALC is the standard tool in CLAN for analyzing
//! fluency in stuttering research.
//!
//! Disfluency categories detected:
//!
//! **Stuttering-Like Disfluencies (SLD):**
//! - Prolongations (`:` within a word, e.g., `wa:nt`)
//! - Broken words (`^` notation)
//! - Blocks (not yet fully implemented)
//! - Part-word repetitions (not yet fully implemented)
//! - Whole-word repetitions (consecutive identical words)
//!
//! **Typical Disfluencies (TD):**
//! - Phrase repetitions (`[/]`)
//! - Revisions (`[//]`)
//! - Filled pauses (`&-uh`, `&-um`, etc.)
//! - Phonological fragments (`&+` prefix)
//!
//! All counts are reported as raw values and as percentages per 100 words.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409273)
//! for the original FLUCALC command specification.
//!
//! # Differences from CLAN
//!
//! - Detection is based on recursive AST traversal rather than serialized
//!   CHAT text scanning.
//! - Part-word repetitions and blocks are counted via CHAT notation markers
//!   rather than acoustic analysis.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::{
    BracketedItem, ContentAnnotation, RetraceKind, Utterance, UtteranceContent, WordCategory,
};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    UtteranceCount, WordCount,
};

/// Configuration for the FLUCALC command.
#[derive(Debug, Clone, Default)]
pub struct FlucalcConfig {
    /// Use syllable-based metrics instead of word-based.
    pub syllable_mode: bool,
}

/// Per-speaker fluency metrics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SpeakerFluency {
    /// Speaker identifier.
    pub speaker: String,
    /// Total utterances analyzed.
    pub utterances: UtteranceCount,
    /// Total words produced.
    pub total_words: WordCount,

    // Stuttering-Like Disfluencies (SLD)
    /// Prolongations (`:` in word).
    pub prolongations: u64,
    /// Broken words (`^` notation).
    pub broken_words: u64,
    /// Blocks (`≠` notation).
    pub blocks: u64,
    /// Part-word repetitions (PWR).
    pub part_word_reps: u64,
    /// Whole-word repetitions (WWR).
    pub whole_word_reps: u64,

    // Typical Disfluencies (TD)
    /// Phrase repetitions `[/]`.
    pub phrase_reps: u64,
    /// Word/phrase revisions `[//]`.
    pub revisions: u64,
    /// Filled pauses (`&-uh`, `&-um`, etc.).
    pub filled_pauses: u64,
    /// Phonological fragments (`&+`).
    pub phon_fragments: u64,
}

impl SpeakerFluency {
    /// Total stuttering-like disfluencies.
    pub fn total_sld(&self) -> u64 {
        self.prolongations
            + self.broken_words
            + self.blocks
            + self.part_word_reps
            + self.whole_word_reps
    }

    /// Total typical disfluencies.
    pub fn total_td(&self) -> u64 {
        self.phrase_reps + self.revisions + self.filled_pauses + self.phon_fragments
    }

    /// Total disfluencies.
    pub fn total_disfluencies(&self) -> u64 {
        self.total_sld() + self.total_td()
    }

    /// SLD percentage (per 100 words).
    pub fn sld_pct(&self) -> f64 {
        if self.total_words > 0 {
            self.total_sld() as f64 / self.total_words as f64 * 100.0
        } else {
            0.0
        }
    }

    /// TD percentage (per 100 words).
    pub fn td_pct(&self) -> f64 {
        if self.total_words > 0 {
            self.total_td() as f64 / self.total_words as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// Typed output for the FLUCALC command.
#[derive(Debug, Clone, Serialize)]
pub struct FlucalcResult {
    /// Per-speaker fluency data.
    pub speakers: Vec<SpeakerFluency>,
}

impl FlucalcResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("flucalc");
        for sp in &self.speakers {
            let rows = vec![
                TableRow {
                    values: vec!["Prolongations".to_owned(), sp.prolongations.to_string()],
                },
                TableRow {
                    values: vec!["Broken words".to_owned(), sp.broken_words.to_string()],
                },
                TableRow {
                    values: vec!["Blocks".to_owned(), sp.blocks.to_string()],
                },
                TableRow {
                    values: vec!["Part-word reps".to_owned(), sp.part_word_reps.to_string()],
                },
                TableRow {
                    values: vec!["Whole-word reps".to_owned(), sp.whole_word_reps.to_string()],
                },
                TableRow {
                    values: vec!["Total SLD".to_owned(), sp.total_sld().to_string()],
                },
                TableRow {
                    values: vec!["Phrase reps".to_owned(), sp.phrase_reps.to_string()],
                },
                TableRow {
                    values: vec!["Revisions".to_owned(), sp.revisions.to_string()],
                },
                TableRow {
                    values: vec!["Filled pauses".to_owned(), sp.filled_pauses.to_string()],
                },
                TableRow {
                    values: vec!["Phon fragments".to_owned(), sp.phon_fragments.to_string()],
                },
                TableRow {
                    values: vec!["Total TD".to_owned(), sp.total_td().to_string()],
                },
                TableRow {
                    values: vec!["Total words".to_owned(), sp.total_words.to_string()],
                },
                TableRow {
                    values: vec!["SLD %".to_owned(), format!("{:.1}%", sp.sld_pct())],
                },
                TableRow {
                    values: vec!["TD %".to_owned(), format!("{:.1}%", sp.td_pct())],
                },
            ];
            let section = Section::with_table(
                format!("Speaker: {}", sp.speaker),
                vec!["Metric".to_owned(), "Value".to_owned()],
                rows,
            );
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for FlucalcResult {
    /// Render fluency metrics as a human-readable text table.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render fluency metrics in CLAN-compatible format.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for sp in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", sp.speaker));
            out.push_str(&format!("  Utterances:      {}\n", sp.utterances));
            out.push_str(&format!("  Total words:     {}\n", sp.total_words));
            out.push_str("  --- SLD ---\n");
            out.push_str(&format!("  Prolongations:   {}\n", sp.prolongations));
            out.push_str(&format!("  Broken words:    {}\n", sp.broken_words));
            out.push_str(&format!("  Blocks:          {}\n", sp.blocks));
            out.push_str(&format!("  Part-word reps:  {}\n", sp.part_word_reps));
            out.push_str(&format!("  Whole-word reps: {}\n", sp.whole_word_reps));
            out.push_str(&format!(
                "  SLD total:       {} ({:.1}%)\n",
                sp.total_sld(),
                sp.sld_pct()
            ));
            out.push_str("  --- TD ---\n");
            out.push_str(&format!("  Phrase reps:     {}\n", sp.phrase_reps));
            out.push_str(&format!("  Revisions:       {}\n", sp.revisions));
            out.push_str(&format!("  Filled pauses:   {}\n", sp.filled_pauses));
            out.push_str(&format!("  Phon fragments:  {}\n", sp.phon_fragments));
            out.push_str(&format!(
                "  TD total:        {} ({:.1}%)\n",
                sp.total_td(),
                sp.td_pct()
            ));
            out.push('\n');
        }
        out
    }
}

/// Accumulated state for FLUCALC across all files.
#[derive(Debug, Default)]
pub struct FlucalcState {
    /// Per-speaker fluency data.
    speakers: BTreeMap<String, SpeakerFluency>,
}

/// FLUCALC command implementation.
///
/// Processes each utterance by serializing the main tier to CHAT text and
/// scanning for disfluency markers. Results are accumulated per speaker.
pub struct FlucalcCommand {
    _config: FlucalcConfig,
}

impl FlucalcCommand {
    /// Create a new FLUCALC command with the given config.
    pub fn new(config: FlucalcConfig) -> Self {
        Self { _config: config }
    }
}

/// Check if a word contains a prolongation marker (`:` used for stretching).
fn has_prolongation(word: &str) -> bool {
    // In CHAT, prolongation is marked by `:` within a word (not in speaker codes)
    word.contains(':') && !word.starts_with('*') && !word.starts_with('%')
}

/// Check if a word is a broken word (contains `^`).
fn has_broken_word(word: &str) -> bool {
    word.contains('^')
}

/// Count disfluencies in the main tier AST.
fn count_disfluencies(content: &[UtteranceContent], fluency: &mut SpeakerFluency) {
    let mut prev_word: Option<String> = None;
    count_disfluencies_content(content, fluency, &mut prev_word);
}

fn count_disfluencies_content(
    content: &[UtteranceContent],
    fluency: &mut SpeakerFluency,
    prev_word: &mut Option<String>,
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                count_word(word.raw_text(), word.category.as_ref(), fluency, prev_word);
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                count_scoped_annotations(&annotated.scoped_annotations, fluency);
                count_word(
                    annotated.inner.raw_text(),
                    annotated.inner.category.as_ref(),
                    fluency,
                    prev_word,
                );
            }
            UtteranceContent::ReplacedWord(replaced) => {
                count_scoped_annotations(&replaced.scoped_annotations, fluency);
                count_word(
                    replaced.word.raw_text(),
                    replaced.word.category.as_ref(),
                    fluency,
                    prev_word,
                );
            }
            UtteranceContent::Group(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                count_scoped_annotations(&annotated.scoped_annotations, fluency);
                count_disfluencies_bracketed(&annotated.inner.content.content, fluency, prev_word);
            }
            UtteranceContent::PhoGroup(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            UtteranceContent::SinGroup(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            UtteranceContent::Quotation(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            UtteranceContent::Retrace(retrace) => {
                count_retrace_kind(retrace.kind, fluency);
                count_disfluencies_bracketed(&retrace.content.content, fluency, prev_word);
            }
            _ => {}
        }
    }
}

fn count_disfluencies_bracketed(
    items: &[BracketedItem],
    fluency: &mut SpeakerFluency,
    prev_word: &mut Option<String>,
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                count_word(word.raw_text(), word.category.as_ref(), fluency, prev_word);
            }
            BracketedItem::AnnotatedWord(annotated) => {
                count_scoped_annotations(&annotated.scoped_annotations, fluency);
                count_word(
                    annotated.inner.raw_text(),
                    annotated.inner.category.as_ref(),
                    fluency,
                    prev_word,
                );
            }
            BracketedItem::ReplacedWord(replaced) => {
                count_scoped_annotations(&replaced.scoped_annotations, fluency);
                count_word(
                    replaced.word.raw_text(),
                    replaced.word.category.as_ref(),
                    fluency,
                    prev_word,
                );
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                count_scoped_annotations(&annotated.scoped_annotations, fluency);
                count_disfluencies_bracketed(&annotated.inner.content.content, fluency, prev_word);
            }
            BracketedItem::PhoGroup(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            BracketedItem::SinGroup(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            BracketedItem::Quotation(group) => {
                count_disfluencies_bracketed(&group.content.content, fluency, prev_word);
            }
            BracketedItem::Retrace(retrace) => {
                count_retrace_kind(retrace.kind, fluency);
                count_disfluencies_bracketed(&retrace.content.content, fluency, prev_word);
            }
            _ => {}
        }
    }
}

/// Count retrace-type disfluencies from first-class Retrace content.
///
/// `[/]` (Partial) counts as a phrase repetition; `[//]` (Full) counts as
/// a revision. Other retrace kinds are not currently counted by FLUCALC.
fn count_retrace_kind(kind: RetraceKind, fluency: &mut SpeakerFluency) {
    match kind {
        RetraceKind::Partial => fluency.phrase_reps += 1,
        RetraceKind::Full => fluency.revisions += 1,
        RetraceKind::Multiple | RetraceKind::Reformulation => {}
    }
}

fn count_scoped_annotations(_annotations: &[ContentAnnotation], _fluency: &mut SpeakerFluency) {
    // Retrace markers are now Retrace content variants, not ContentAnnotation.
    // Retained for future non-retrace annotation counting if needed.
}

fn count_word(
    word: &str,
    category: Option<&WordCategory>,
    fluency: &mut SpeakerFluency,
    prev_word: &mut Option<String>,
) {
    match category {
        Some(WordCategory::Filler) => {
            fluency.filled_pauses += 1;
            return;
        }
        Some(WordCategory::PhonologicalFragment) => {
            fluency.phon_fragments += 1;
            return;
        }
        Some(WordCategory::Omission | WordCategory::CAOmission) => return,
        _ => {}
    }

    if has_prolongation(word) {
        fluency.prolongations += 1;
    }

    if has_broken_word(word) {
        fluency.broken_words += 1;
    }

    let current = normalize_repetition_word(word);
    if let Some(prev) = prev_word.as_ref()
        && !current.is_empty()
        && prev == &current
    {
        fluency.whole_word_reps += 1;
    }

    fluency.total_words += 1;
    *prev_word = Some(current);
}

fn normalize_repetition_word(word: &str) -> String {
    word.to_lowercase().chars().filter(|c| *c != ':').collect()
}

impl AnalysisCommand for FlucalcCommand {
    type Config = FlucalcConfig;
    type State = FlucalcState;
    type Output = FlucalcResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();
        let fluency = state
            .speakers
            .entry(speaker.clone())
            .or_insert_with(|| SpeakerFluency {
                speaker,
                ..Default::default()
            });

        fluency.utterances += 1;

        count_disfluencies(&utterance.main.content.content, fluency);
    }

    fn finalize(&self, state: Self::State) -> FlucalcResult {
        let speakers: Vec<SpeakerFluency> = state.speakers.into_values().collect();
        FlucalcResult { speakers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Line;

    fn parse_content(chat: &str) -> Vec<UtteranceContent> {
        let parsed = talkbank_transform::parse_and_validate(
            chat,
            talkbank_model::ParseValidateOptions::default(),
        )
        .unwrap();
        parsed
            .lines
            .into_iter()
            .find_map(|line| match line {
                Line::Utterance(utt) => Some(utt.main.content.content.0),
                _ => None,
            })
            .expect("expected utterance")
    }

    #[test]
    fn flucalc_empty() {
        let cmd = FlucalcCommand::new(FlucalcConfig::default());
        let state = FlucalcState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }

    #[test]
    fn count_filled_pauses() {
        let mut fluency = SpeakerFluency::default();
        let content = parse_content(
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tI &-um want &-uh that .\n@End\n",
        );
        count_disfluencies(&content, &mut fluency);
        assert_eq!(fluency.filled_pauses, 2);
        assert_eq!(fluency.total_words, 3);
    }

    #[test]
    fn count_prolongations() {
        let mut fluency = SpeakerFluency::default();
        let content =
            parse_content("@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tI wa:nt that .\n@End\n");
        count_disfluencies(&content, &mut fluency);
        assert_eq!(fluency.prolongations, 1);
        assert_eq!(fluency.total_words, 3);
    }

    #[test]
    fn count_phrase_reps_and_revisions() {
        let mut fluency = SpeakerFluency::default();
        let content = parse_content(
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\t<I want> [/] want <that> [//] this .\n@End\n",
        );
        count_disfluencies(&content, &mut fluency);
        assert_eq!(fluency.phrase_reps, 1);
        assert_eq!(fluency.revisions, 1);
    }

    #[test]
    fn sld_td_percentages() {
        let sp = SpeakerFluency {
            total_words: 100,
            prolongations: 3,
            whole_word_reps: 2,
            filled_pauses: 5,
            revisions: 3,
            ..Default::default()
        };
        assert_eq!(sp.total_sld(), 5);
        assert_eq!(sp.total_td(), 8);
        assert!((sp.sld_pct() - 5.0).abs() < f64::EPSILON);
        assert!((sp.td_pct() - 8.0).abs() < f64::EPSILON);
    }
}
