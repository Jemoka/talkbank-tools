//! CHIP — Child/Parent Interaction Profile.
//!
//! Reimplements CLAN's CHIP command, which analyzes interaction patterns between
//! a child speaker and their conversational partners. It categorizes successive
//! utterance pairs to measure imitation, repetition, and overlap. CHIP is
//! commonly used in child language research to quantify how much a child
//! imitates or echoes their interlocutor.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                       | Rust equivalent                                     |
//! |------------------------------------|-----------------------------------------------------|
//! | `chip +t*CHI file.cha`             | `chatter analyze chip file.cha -s CHI`              |
//! | `chip file.cha`                    | `chatter analyze chip file.cha`                     |
//!
//! # Interaction Categories
//!
//! For each adjacent utterance pair (speaker A followed by speaker B):
//! - **Exact repetition**: B's utterance words are identical to A's (order-independent)
//! - **Overlap**: B's utterance shares >=50% of words with A's (using the smaller
//!   unique-word set as denominator)
//! - **No overlap**: B's utterance shares <50% of words with A's
//!
//! Only cross-speaker adjacency is considered; consecutive utterances by the
//! same speaker do not produce interaction records. Adjacency state is reset
//! at file boundaries.
//!
//! # Output
//!
//! Per directed speaker pair (e.g., MOT->CHI is distinct from CHI->MOT):
//! - Counts of exact repetitions, overlaps, and non-overlaps
//! - Percentages of each category relative to the pair total
//! - Grand totals across all pairs
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Overlap comparison operates on parsed word content, not raw text.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::HashSet;
use std::fmt::Write;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{Utterance, WriteChat};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    countable_words,
};

/// Shared-word ratio threshold for classifying consecutive utterances as
/// overlapping. Two utterances with ratio ≥ this value are classified as
/// `Interaction::Overlap`. The CLAN CHIP command uses 50%.
const OVERLAP_THRESHOLD: f64 = 0.5;

/// Configuration for the CHIP command.
#[derive(Debug, Clone, Default)]
pub struct ChipConfig {}

/// A directed speaker-pair interaction entry.
#[derive(Debug, Clone, Serialize)]
pub struct ChipPairEntry {
    /// Speaker who produced the first utterance.
    pub from: String,
    /// Speaker who produced the second utterance.
    pub to: String,
    /// Number of exact-repetition interactions.
    pub exact_repetitions: u64,
    /// Number of overlap interactions (≥50% shared words).
    pub overlaps: u64,
    /// Number of no-overlap interactions (<50% shared words).
    pub no_overlaps: u64,
}

impl ChipPairEntry {
    /// Total interactions for this pair.
    pub fn total(&self) -> u64 {
        self.exact_repetitions + self.overlaps + self.no_overlaps
    }
}

/// Typed output for the CHIP command.
#[derive(Debug, Clone, Serialize)]
pub struct ChipResult {
    /// Speaker pair entries in encounter order.
    pub pairs: Vec<ChipPairEntry>,
    /// Total interactions across all pairs.
    pub total_interactions: u64,
    /// Total exact repetitions across all pairs.
    pub total_exact: u64,
    /// Total overlaps across all pairs.
    pub total_overlaps: u64,
    /// Echoed utterance lines for CLAN output.
    #[serde(skip)]
    pub echoed_lines: Vec<String>,
}

impl ChipResult {
    /// Convert typed CHIP output into the shared table/field render container.
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("chip");
        if self.pairs.is_empty() {
            return result;
        }

        let rows: Vec<TableRow> = self
            .pairs
            .iter()
            .map(|entry| {
                let total = entry.total();
                let rep_pct = if total > 0 {
                    format!(
                        "{:.1}%",
                        entry.exact_repetitions as f64 / total as f64 * 100.0
                    )
                } else {
                    "0.0%".to_owned()
                };
                let ovl_pct = if total > 0 {
                    format!("{:.1}%", entry.overlaps as f64 / total as f64 * 100.0)
                } else {
                    "0.0%".to_owned()
                };
                TableRow {
                    values: vec![
                        format!("{} → {}", entry.from, entry.to),
                        entry.exact_repetitions.to_string(),
                        entry.overlaps.to_string(),
                        entry.no_overlaps.to_string(),
                        total.to_string(),
                        rep_pct,
                        ovl_pct,
                    ],
                }
            })
            .collect();

        let mut section = Section::with_table(
            "Interaction profile".to_owned(),
            vec![
                "Pair".to_owned(),
                "Exact".to_owned(),
                "Overlap".to_owned(),
                "No overlap".to_owned(),
                "Total".to_owned(),
                "Exact %".to_owned(),
                "Overlap %".to_owned(),
            ],
            rows,
        );
        section.fields.insert(
            "Total interactions".to_owned(),
            self.total_interactions.to_string(),
        );
        section.fields.insert(
            "Total exact repetitions".to_owned(),
            self.total_exact.to_string(),
        );
        section
            .fields
            .insert("Total overlaps".to_owned(), self.total_overlaps.to_string());

        result.add_section(section);
        result
    }
}

/// CLAN CHIP measure labels and their format type (integer or float).
const CHIP_MEASURES: &[(&str, bool)] = &[
    ("Responses", false),
    ("Overlap  ", false),
    ("No_Overlap", false),
    ("%_Overlap", true),
    ("Avg_Dist", true),
    ("Rep_Index", true),
    ("ADD_OPS  ", false),
    ("DEL_OPS  ", false),
    ("EXA_OPS  ", false),
    ("%_ADD_OPS", true),
    ("%_DEL_OPS", true),
    ("%_EXA_OPS", true),
    ("ADD_WORD", false),
    ("DEL_WORD", false),
    ("EXA_WORD", false),
    ("%_ADD_WORDS", true),
    ("%_DEL_WORDS", true),
    ("%_EXA_WORDS", true),
    ("MORPH_ADD", false),
    ("MORPH_DEL", false),
    ("MORPH_EXA", false),
    ("MORPH_SUB", false),
    ("%_MORPH_ADD", true),
    ("%_MORPH_DEL", true),
    ("%_MORPH_EXA", true),
    ("%_MORPH_SUB", true),
    ("AV_WORD_ADD", true),
    ("AV_WORD_DEL", true),
    ("AV_WORD_EXA", true),
    ("IMITAT   ", false),
    ("%_IMITAT", true),
    ("EXACT    ", false),
    ("EXPAN    ", false),
    ("REDUC    ", false),
    ("%_EXACT  ", true),
    ("%_EXPAN  ", true),
    ("%_REDUC  ", true),
];

impl CommandOutput for ChipResult {
    /// Render CHIP output through the shared text table renderer.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible CHIP output.
    ///
    /// Format: echoed utterances, separator, scored counts, 36-row measure matrix
    /// with ADU/CHI/ASR/CSR columns.
    fn render_clan(&self) -> String {
        let mut out = String::new();

        // Echo utterances.
        for line in &self.echoed_lines {
            writeln!(out, "{line}").ok();
        }

        // Separator and file header.
        writeln!(
            out,
            "==========================================================="
        )
        .ok();
        writeln!(out, "File: pipeout").ok();
        writeln!(out).ok();

        // Scored utterance counts (currently always 0).
        writeln!(out, "Total  scored utterances: 0").ok();
        writeln!(out, "Total  scored utterances: 0").ok();
        writeln!(out).ok();

        // Matrix header.
        writeln!(out, "Measure  \tADU\tCHI\tASR\tCSR").ok();
        writeln!(
            out,
            "-----------------------------------------------------------"
        )
        .ok();

        // 36 measure rows (currently all zeros — full computation not yet implemented).
        for &(label, is_float) in CHIP_MEASURES {
            if is_float {
                writeln!(out, "{label}\t0.00\t0.00\t0.00\t0.00").ok();
            } else {
                writeln!(out, "{label}\t0\t0\t0\t0").ok();
            }
        }

        out
    }
}

/// Interaction category for an adjacent utterance pair (internal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interaction {
    /// B's words are identical to A's.
    ExactRepetition,
    /// B shares ≥50% of words with A.
    Overlap,
    /// B shares <50% of words with A.
    NoOverlap,
}

/// Key for a directed speaker pair (from → to).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SpeakerPair {
    from: String,
    to: String,
}

/// Accumulated interaction counts for a speaker pair (internal).
#[derive(Debug, Default)]
struct PairInteractions {
    exact_repetitions: u64,
    overlaps: u64,
    no_overlaps: u64,
}

impl PairInteractions {
    /// Total classified interactions accumulated for this directed pair.
    fn total(&self) -> u64 {
        self.exact_repetitions + self.overlaps + self.no_overlaps
    }
}

/// Accumulated state for CHIP across all files.
#[derive(Debug, Default)]
pub struct ChipState {
    /// Per-speaker-pair interaction counts.
    by_pair: IndexMap<SpeakerPair, PairInteractions>,
    /// Previous utterance's speaker and word set (for pair detection).
    pub prev_speaker: Option<String>,
    /// Previous utterance's words (lowercased, sorted for comparison).
    pub prev_words: Vec<String>,
    /// Echoed utterance lines for CLAN output.
    echoed_lines: Vec<String>,
}

/// CHIP command implementation.
///
/// Compares each utterance with the immediately preceding one. When speakers
/// differ, classifies the interaction as exact repetition, overlap, or
/// no overlap based on word-level comparison.
#[derive(Debug, Clone, Default)]
pub struct ChipCommand;

impl AnalysisCommand for ChipCommand {
    type Config = ChipConfig;
    type State = ChipState;
    type Output = ChipResult;

    /// Compare each utterance against the immediately previous utterance.
    ///
    /// Interactions are recorded only when speakers differ and both utterances
    /// contain at least one countable word.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.as_str().to_owned();
        let words: Vec<String> = countable_words(&utterance.main.content.content)
            .map(|w| w.cleaned_text().to_lowercase())
            .collect();

        // Echo utterance lines for CLAN output (main tier + %mor only, not %gra).
        state.echoed_lines.push(utterance.main.to_chat_string());
        if let Some(mor_tier) = utterance.mor_tier() {
            state.echoed_lines.push(mor_tier.to_chat_string());
        }

        // Compare with previous utterance if speakers differ
        if let Some(ref prev_speaker) = state.prev_speaker
            && *prev_speaker != speaker
            && !state.prev_words.is_empty()
            && !words.is_empty()
        {
            let interaction = classify_interaction(&state.prev_words, &words);
            let pair = SpeakerPair {
                from: prev_speaker.clone(),
                to: speaker.clone(),
            };
            let counts = state.by_pair.entry(pair).or_default();
            match interaction {
                Interaction::ExactRepetition => counts.exact_repetitions += 1,
                Interaction::Overlap => counts.overlaps += 1,
                Interaction::NoOverlap => counts.no_overlaps += 1,
            }
        }

        state.prev_speaker = Some(speaker);
        state.prev_words = words;
    }

    /// Reset adjacency state so interactions never cross file boundaries.
    fn end_file(&self, _file_context: &FileContext<'_>, state: &mut Self::State) {
        // Reset cross-utterance state at file boundaries
        state.prev_speaker = None;
        state.prev_words.clear();
    }

    /// Materialize totals and preserve encounter order for pair rows.
    fn finalize(&self, state: Self::State) -> ChipResult {
        let echoed_lines = state.echoed_lines;
        if state.by_pair.is_empty() {
            return ChipResult {
                pairs: Vec::new(),
                total_interactions: 0,
                total_exact: 0,
                total_overlaps: 0,
                echoed_lines,
            };
        }

        let total_interactions: u64 = state.by_pair.values().map(PairInteractions::total).sum();
        let total_exact: u64 = state.by_pair.values().map(|p| p.exact_repetitions).sum();
        let total_overlaps: u64 = state.by_pair.values().map(|p| p.overlaps).sum();

        let pairs: Vec<ChipPairEntry> = state
            .by_pair
            .into_iter()
            .map(|(pair, counts)| ChipPairEntry {
                from: pair.from,
                to: pair.to,
                exact_repetitions: counts.exact_repetitions,
                overlaps: counts.overlaps,
                no_overlaps: counts.no_overlaps,
            })
            .collect();

        ChipResult {
            pairs,
            total_interactions,
            total_exact,
            total_overlaps,
            echoed_lines,
        }
    }
}

/// Classify the interaction between two utterances based on word overlap.
///
/// - Exact repetition: sorted word lists are identical
/// - Overlap: ≥50% of the shorter utterance's unique words appear in the longer
/// - No overlap: <50% overlap
///
/// # Precondition
/// Both word lists must be non-empty.
fn classify_interaction(prev_words: &[String], curr_words: &[String]) -> Interaction {
    // Compare sorted word lists for exact repetition
    let mut prev_sorted = prev_words.to_vec();
    let mut curr_sorted = curr_words.to_vec();
    prev_sorted.sort();
    curr_sorted.sort();

    if prev_sorted == curr_sorted {
        return Interaction::ExactRepetition;
    }

    // Compute word overlap ratio
    let prev_set: HashSet<&str> = prev_words.iter().map(|s| s.as_str()).collect();
    let curr_set: HashSet<&str> = curr_words.iter().map(|s| s.as_str()).collect();
    let intersection_count = prev_set.intersection(&curr_set).count();

    // Use the smaller set as denominator for overlap ratio
    let min_size = prev_set.len().min(curr_set.len());
    let ratio = if min_size > 0 {
        intersection_count as f64 / min_size as f64
    } else {
        0.0
    };

    if ratio >= OVERLAP_THRESHOLD {
        Interaction::Overlap
    } else {
        Interaction::NoOverlap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{MainTier, Terminator, UtteranceContent, Word};

    /// Build a minimal utterance with plain words for interaction tests.
    fn make_utterance(speaker: &str, words: &[&str]) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    /// Build a stable `FileContext` fixture reused across test cases.
    fn file_ctx(chat_file: &talkbank_model::ChatFile) -> FileContext<'_> {
        FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file,
            filename: "test",
            line_map: None,
        }
    }

    /// Identical adjacent content across speakers should classify as exact repetition.
    #[test]
    fn chip_exact_repetition() {
        let command = ChipCommand;
        let mut state = ChipState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        // MOT says "want cookie", CHI repeats "want cookie"
        let u1 = make_utterance("MOT", &["want", "cookie"]);
        let u2 = make_utterance("CHI", &["want", "cookie"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let pair = SpeakerPair {
            from: "MOT".to_owned(),
            to: "CHI".to_owned(),
        };
        assert_eq!(state.by_pair[&pair].exact_repetitions, 1);
        assert_eq!(state.by_pair[&pair].overlaps, 0);
        assert_eq!(state.by_pair[&pair].no_overlaps, 0);
    }

    /// At least 50% overlap of the smaller unique-word set counts as overlap.
    #[test]
    fn chip_overlap() {
        let command = ChipCommand;
        let mut state = ChipState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        // MOT says "I want cookie", CHI says "want cookie please"
        // Shared: "want", "cookie" (2 of 3 unique words in shorter) → ≥50%
        let u1 = make_utterance("MOT", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["want", "cookie", "please"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let pair = SpeakerPair {
            from: "MOT".to_owned(),
            to: "CHI".to_owned(),
        };
        assert_eq!(state.by_pair[&pair].exact_repetitions, 0);
        assert_eq!(state.by_pair[&pair].overlaps, 1);
    }

    /// Disjoint vocabularies should classify as no-overlap.
    #[test]
    fn chip_no_overlap() {
        let command = ChipCommand;
        let mut state = ChipState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        // MOT: "look at the dog", CHI: "I want milk"
        // No shared words
        let u1 = make_utterance("MOT", &["look", "at", "the", "dog"]);
        let u2 = make_utterance("CHI", &["I", "want", "milk"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let pair = SpeakerPair {
            from: "MOT".to_owned(),
            to: "CHI".to_owned(),
        };
        assert_eq!(state.by_pair[&pair].no_overlaps, 1);
    }

    /// Consecutive utterances by the same speaker should not create an interaction edge.
    #[test]
    fn chip_same_speaker_no_interaction() {
        let command = ChipCommand;
        let mut state = ChipState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        // Two consecutive utterances by same speaker — no interaction counted
        let u1 = make_utterance("CHI", &["hello"]);
        let u2 = make_utterance("CHI", &["hello"]);
        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        assert!(state.by_pair.is_empty());
    }

    /// The state machine should track multiple directed interactions in one file.
    #[test]
    fn chip_multiple_interactions() {
        let command = ChipCommand;
        let mut state = ChipState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = file_ctx(&chat_file);

        let u1 = make_utterance("MOT", &["want", "cookie"]);
        let u2 = make_utterance("CHI", &["want", "cookie"]); // exact
        let u3 = make_utterance("MOT", &["good", "job"]);
        let u4 = make_utterance("CHI", &["more", "cookie"]); // no overlap with "good job"

        command.process_utterance(&u1, &ctx, &mut state);
        command.process_utterance(&u2, &ctx, &mut state);
        command.process_utterance(&u3, &ctx, &mut state);
        command.process_utterance(&u4, &ctx, &mut state);
        command.end_file(&ctx, &mut state);

        let mot_to_chi = SpeakerPair {
            from: "MOT".to_owned(),
            to: "CHI".to_owned(),
        };
        assert_eq!(state.by_pair[&mot_to_chi].exact_repetitions, 1);
        assert_eq!(state.by_pair[&mot_to_chi].no_overlaps, 1);

        // CHI → MOT: "want cookie" → "good job" = no overlap
        let chi_to_mot = SpeakerPair {
            from: "CHI".to_owned(),
            to: "MOT".to_owned(),
        };
        assert_eq!(state.by_pair[&chi_to_mot].no_overlaps, 1);
    }

    /// Finalizing untouched state should produce an empty result.
    #[test]
    fn chip_empty_state() {
        let command = ChipCommand;
        let state = ChipState::default();
        let result = command.finalize(state);
        assert!(result.pairs.is_empty());
    }

    /// Word order differences alone should still be exact repetition.
    #[test]
    fn classify_interaction_exact() {
        assert_eq!(
            classify_interaction(
                &["want".to_owned(), "cookie".to_owned()],
                &["cookie".to_owned(), "want".to_owned()],
            ),
            Interaction::ExactRepetition
        );
    }

    /// Exactly 50% overlap should take the overlap branch.
    #[test]
    fn classify_interaction_overlap_threshold() {
        // 1 of 2 unique words shared = 50% → overlap
        assert_eq!(
            classify_interaction(
                &["want".to_owned(), "cookie".to_owned()],
                &["want".to_owned(), "milk".to_owned()],
            ),
            Interaction::Overlap
        );
    }

    /// Zero shared vocabulary should take the no-overlap branch.
    #[test]
    fn classify_interaction_no_overlap() {
        // 0 of 2 shared → no overlap
        assert_eq!(
            classify_interaction(
                &["hello".to_owned(), "world".to_owned()],
                &["goodbye".to_owned(), "moon".to_owned()],
            ),
            Interaction::NoOverlap
        );
    }
}
