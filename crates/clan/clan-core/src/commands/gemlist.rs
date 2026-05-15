// `unreachable!()` arms used here are guarded by `matches!` checks
// in the surrounding `match` patterns — the let-else cannot fail.
// `writeln!` to String is infallible.
#![allow(clippy::unwrap_used, clippy::unreachable)]

//! GEMLIST — List Gem Segments.
//!
//! Lists all gem segments (`@Bg`/`@Eg` bracketed regions) found in CHAT files,
//! reporting the label, utterance count, and participating speakers for each gem.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409206)
//! for the original GEM command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                    | Rust equivalent                                |
//! |---------------------------------|------------------------------------------------|
//! | `gem file.cha`                  | `chatter analyze gemlist file.cha`             |
//! | `gem +t*CHI file.cha`           | `chatter analyze gemlist file.cha -s CHI`      |
//!
//! # Output
//!
//! Per gem label:
//! - Number of utterances within the gem scope
//! - Number of occurrences (how many `@Bg`/`@Eg` pairs with this label)
//! - Speakers who produced utterances within the gem
//! - Source files containing this gem
//!
//! # Implementation Note
//!
//! Gem boundaries (`@Bg`/`@Eg`) are interleaved headers in `ChatFile.lines`.
//! Since the parser does not populate `Utterance.preceding_headers`, this
//! command scans the full line array in `end_file()` rather than relying
//! on per-utterance callbacks.
//!
//! # Differences from CLAN
//!
//! - Gem boundary detection operates on parsed `Header` variants from the
//!   AST rather than raw text line matching.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::HashSet;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{Header, Line, Utterance};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    UtteranceCount,
};

/// Configuration for the GEMLIST command.
#[derive(Debug, Clone, Default)]
pub struct GemlistConfig {}

/// A single gem segment's aggregated data.
#[derive(Debug, Clone, Serialize)]
pub struct GemEntry {
    /// Gem label (the value after `@Bg:`/`@Eg:`).
    pub label: String,
    /// Number of `@Bg` occurrences with this label.
    pub occurrences: u64,
    /// Total utterances within all instances of this gem.
    pub utterance_count: UtteranceCount,
    /// Speaker codes who produced utterances within this gem (sorted).
    pub speakers: Vec<String>,
}

/// Typed output for the GEMLIST command.
#[derive(Debug, Clone, Serialize)]
pub struct GemlistResult {
    /// Gem entries in encounter order.
    pub gems: Vec<GemEntry>,
    /// Total `@Bg` occurrences across all gem labels.
    pub total_occurrences: u64,
    /// Total utterances inside any gem scope.
    pub total_utterances: UtteranceCount,
}

impl GemlistResult {
    /// Convert typed gem aggregates into the shared section/table render model.
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("gemlist");
        if self.gems.is_empty() {
            return result;
        }

        let rows: Vec<TableRow> = self
            .gems
            .iter()
            .map(|g| TableRow {
                values: vec![
                    g.label.clone(),
                    g.occurrences.to_string(),
                    g.utterance_count.to_string(),
                    g.speakers.join(", "),
                ],
            })
            .collect();

        let mut section = Section::with_table(
            "Gem segments".to_owned(),
            vec![
                "Label".to_owned(),
                "Occurrences".to_owned(),
                "Utterances".to_owned(),
                "Speakers".to_owned(),
            ],
            rows,
        );
        section
            .fields
            .insert("Total gems".to_owned(), self.gems.len().to_string());
        section.fields.insert(
            "Total occurrences".to_owned(),
            self.total_occurrences.to_string(),
        );
        section.fields.insert(
            "Total utterances in gems".to_owned(),
            self.total_utterances.to_string(),
        );

        result.add_section(section);
        result
    }
}

impl CommandOutput for GemlistResult {
    /// Render via the shared tabular text formatter.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }
}

/// Accumulated data for a single gem label (internal).
#[derive(Debug, Default)]
struct GemInfo {
    /// Total utterances within all instances of this gem.
    utterance_count: u64,
    /// Number of distinct @Bg occurrences for this label.
    occurrence_count: u64,
    /// Speaker codes who produced utterances within this gem.
    speakers: HashSet<String>,
    /// Files containing this gem.
    files: HashSet<String>,
}

/// Accumulated state for GEMLIST across all files.
#[derive(Debug, Default)]
pub struct GemlistState {
    /// Per-label accumulated gem data.
    by_label: IndexMap<String, GemInfo>,
}

/// GEMLIST command implementation.
///
/// Scans `ChatFile.lines` in `end_file()` to find `@Bg`/`@Eg` boundaries
/// and count utterances within each gem scope. This is necessary because
/// the parser stores gem headers as separate `Line::Header` entries rather
/// than attaching them to `Utterance.preceding_headers`.
#[derive(Debug, Clone, Default)]
pub struct GemlistCommand;

impl AnalysisCommand for GemlistCommand {
    type Config = GemlistConfig;
    type State = GemlistState;
    type Output = GemlistResult;

    /// No-op: gem scope tracking runs in `end_file()` over raw line sequence.
    fn process_utterance(
        &self,
        _utterance: &Utterance,
        _file_context: &FileContext<'_>,
        _state: &mut Self::State,
    ) {
        // Gem tracking is done in end_file() by scanning ChatFile.lines directly,
        // because the parser does not populate Utterance.preceding_headers.
    }

    /// Scan interleaved header/utterance lines to accumulate gem boundary stats.
    fn end_file(&self, file_context: &FileContext<'_>, state: &mut Self::State) {
        // Track currently active gem labels (stack for nested gems)
        let mut active_gems: Vec<String> = Vec::new();
        // Track which @Bg labels we've already counted in this file
        let mut seen_begins: HashSet<String> = HashSet::new();

        for line in file_context.chat_file.lines.iter() {
            match line {
                Line::Header { header, .. }
                    if matches!(header.as_ref(), Header::BeginGem { label: Some(_) }) =>
                {
                    let Header::BeginGem { label: Some(label) } = header.as_ref() else {
                        unreachable!()
                    };
                    let label_str = label.as_str().to_owned();
                    active_gems.push(label_str.clone());

                    // Count this as a new occurrence of this gem label
                    let key = format!("{}:{}", file_context.filename, &label_str);
                    if seen_begins.insert(key) {
                        state
                            .by_label
                            .entry(label_str)
                            .or_default()
                            .occurrence_count += 1;
                    }
                }
                Line::Header { header, .. }
                    if matches!(header.as_ref(), Header::EndGem { label: Some(_) }) =>
                {
                    let Header::EndGem { label: Some(label) } = header.as_ref() else {
                        unreachable!()
                    };
                    // Remove the most recent matching @Bg (LIFO)
                    if let Some(pos) = active_gems
                        .iter()
                        .rposition(|g| g.eq_ignore_ascii_case(label.as_str()))
                    {
                        active_gems.remove(pos);
                    }
                }
                Line::Utterance(utterance) if !active_gems.is_empty() => {
                    let speaker = utterance.main.speaker.as_str().to_owned();
                    let filename = file_context.filename.to_owned();

                    for gem_label in &active_gems {
                        let info = state.by_label.entry(gem_label.clone()).or_default();
                        info.utterance_count += 1;
                        info.speakers.insert(speaker.clone());
                        info.files.insert(filename.clone());
                    }
                }
                _ => {}
            }
        }
    }

    /// Materialize per-label gem rows with corpus-wide totals.
    fn finalize(&self, state: Self::State) -> GemlistResult {
        if state.by_label.is_empty() {
            return GemlistResult {
                gems: Vec::new(),
                total_occurrences: 0,
                total_utterances: 0,
            };
        }

        let total_utterances: u64 = state.by_label.values().map(|i| i.utterance_count).sum();
        let total_occurrences: u64 = state.by_label.values().map(|i| i.occurrence_count).sum();

        let gems: Vec<GemEntry> = state
            .by_label
            .into_iter()
            .map(|(label, info)| {
                let mut speakers: Vec<String> = info.speakers.into_iter().collect();
                speakers.sort();
                GemEntry {
                    label,
                    occurrences: info.occurrence_count,
                    utterance_count: info.utterance_count,
                    speakers,
                }
            })
            .collect();

        GemlistResult {
            gems,
            total_occurrences,
            total_utterances,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{ChatFile, GemLabel, MainTier, Terminator, UtteranceContent, Word};

    /// Build a ChatFile with interleaved headers and utterances.
    fn make_chat_file(lines: Vec<Line>) -> ChatFile {
        ChatFile::new(lines)
    }

    /// Build a test utterance line with simple lexical content.
    fn utt_line(speaker: &str, words: &[&str]) -> Line {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Line::utterance(talkbank_model::Utterance::new(main))
    }

    /// Build a `@Bg` header line fixture.
    fn bg_line(label: &str) -> Line {
        Line::header(Header::BeginGem {
            label: Some(GemLabel::new(label)),
        })
    }

    /// Build an `@Eg` header line fixture.
    fn eg_line(label: &str) -> Line {
        Line::header(Header::EndGem {
            label: Some(GemLabel::new(label)),
        })
    }

    /// Utterances between matching `@Bg/@Eg` should be attributed to the gem label.
    #[test]
    fn gemlist_collects_gem_segments() {
        let command = GemlistCommand;
        let mut state = GemlistState::default();

        let chat_file = make_chat_file(vec![
            utt_line("CHI", &["hello"]),        // before gem — not counted
            bg_line("Story"),                   // @Bg:Story
            utt_line("CHI", &["once", "upon"]), // inside gem
            utt_line("MOT", &["a", "time"]),    // inside gem
            eg_line("Story"),                   // @Eg:Story
            utt_line("CHI", &["the", "end"]),   // after gem — not counted
        ]);

        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        command.end_file(&file_ctx, &mut state);
        let result = command.finalize(state);

        assert_eq!(result.gems.len(), 1);
        assert_eq!(result.total_utterances, 2);
        let gem = &result.gems[0];
        assert_eq!(gem.label, "Story");
        assert_eq!(gem.occurrences, 1);
        assert_eq!(gem.utterance_count, 2);
    }

    /// Files without gem headers should produce an empty result.
    #[test]
    fn gemlist_no_gems_empty_result() {
        let command = GemlistCommand;
        let mut state = GemlistState::default();

        let chat_file = make_chat_file(vec![utt_line("CHI", &["hello"])]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        command.end_file(&file_ctx, &mut state);
        let result = command.finalize(state);
        assert!(result.gems.is_empty());
    }

    /// Distinct gem labels in one file should produce separate entries.
    #[test]
    fn gemlist_multiple_gems() {
        let command = GemlistCommand;
        let mut state = GemlistState::default();

        let chat_file = make_chat_file(vec![
            bg_line("Story"),
            utt_line("CHI", &["once"]),
            eg_line("Story"),
            bg_line("Freeplay"),
            utt_line("MOT", &["let's", "play"]),
            eg_line("Freeplay"),
        ]);

        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        command.end_file(&file_ctx, &mut state);
        let result = command.finalize(state);

        assert_eq!(result.gems.len(), 2);
    }

    /// Nested gem scopes should count utterances in both active labels as appropriate.
    #[test]
    fn gemlist_nested_gems() {
        let command = GemlistCommand;
        let mut state = GemlistState::default();

        let chat_file = make_chat_file(vec![
            bg_line("Story"),
            bg_line("Episode"),
            utt_line("CHI", &["hello"]), // in both Story and Episode
            eg_line("Episode"),
            utt_line("CHI", &["bye"]), // in Story only
            eg_line("Story"),
        ]);

        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        command.end_file(&file_ctx, &mut state);
        let result = command.finalize(state);

        assert_eq!(result.gems.len(), 2);

        // Story: "hello" + "bye" = 2 utterances
        let story = result.gems.iter().find(|g| g.label == "Story").unwrap();
        assert_eq!(story.utterance_count, 2);

        // Episode: "hello" = 1 utterance
        let episode = result.gems.iter().find(|g| g.label == "Episode").unwrap();
        assert_eq!(episode.utterance_count, 1);
    }
}
