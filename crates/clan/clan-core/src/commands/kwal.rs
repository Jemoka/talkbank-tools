//! KWAL — Keyword And Line (keyword-in-context search).
//!
//! Searches for utterances containing specified keywords and displays
//! matching lines with context. Keywords are matched as case-insensitive
//! exact words against countable words on the main tier. Wildcards (`*`)
//! are supported for partial matching (e.g., `cook*` matches `cookies`).
//!
//! # CLAN Equivalence
//!
//! | CLAN command                    | Rust equivalent                                  |
//! |---------------------------------|--------------------------------------------------|
//! | `kwal +s"want" file.cha`        | `chatter analyze kwal file.cha -k want`          |
//! | `kwal +s"want" +t*CHI file.cha` | `chatter analyze kwal file.cha -k want -s CHI`   |
//!
//! KWAL does not have a dedicated section in the CLAN manual; it is
//! described alongside other search commands.
//!
//! # Output
//!
//! Each matching utterance with:
//! - Speaker code
//! - Full utterance text
//! - File path (for multi-file searches)
//! - Match count summary per keyword
//!
//! # Differences from CLAN
//!
//! - Search operates on parsed AST word content rather than raw text lines.
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{Utterance, WriteChat};

use crate::framework::word_filter::{countable_words, word_pattern_matches};
use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, NormalizedWord, OutputFormat,
    Section, TableRow,
};

/// Configuration for the KWAL command.
#[derive(Debug, Clone, Default)]
pub struct KwalConfig {
    /// Keywords to search for (case-insensitive exact match, `*` wildcards supported)
    pub keywords: Vec<crate::framework::KeywordPattern>,
}

/// A single match found during KWAL processing.
#[derive(Debug, Clone, Serialize)]
pub struct KwalMatch {
    /// Speaker code.
    pub speaker: String,
    /// Full utterance text (CHAT format).
    pub utterance_text: String,
    /// Source filename.
    pub filename: String,
    /// Matched keyword that triggered this result.
    pub keyword: String,
    /// 1-based line number of this utterance in the source file.
    pub line_number: usize,
}

/// Typed output for the KWAL command.
#[derive(Debug, Clone, Serialize)]
pub struct KwalResult {
    /// All matching utterances in order encountered.
    pub matches: Vec<KwalMatch>,
    /// Per-keyword match counts.
    pub keyword_counts: IndexMap<String, u64>,
}

impl KwalResult {
    /// Convert typed KWAL matches into the shared section/table render model.
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("kwal");

        if !self.matches.is_empty() {
            let rows: Vec<TableRow> = self
                .matches
                .iter()
                .map(|m| TableRow {
                    values: vec![
                        m.filename.clone(),
                        m.speaker.clone(),
                        m.utterance_text.clone(),
                    ],
                })
                .collect();

            let mut matches_section = Section::with_table(
                "Matches".to_owned(),
                vec![
                    "File".to_owned(),
                    "Speaker".to_owned(),
                    "Utterance".to_owned(),
                ],
                rows,
            );
            matches_section
                .fields
                .insert("Total matches".to_owned(), self.matches.len().to_string());
            result.add_section(matches_section);
        }

        if !self.keyword_counts.is_empty() {
            let mut fields = IndexMap::new();
            for (keyword, count) in &self.keyword_counts {
                fields.insert(format!("\"{keyword}\""), count.to_string());
            }
            result.add_section(Section::with_fields("Keyword counts".to_owned(), fields));
        }

        result
    }
}

impl CommandOutput for KwalResult {
    /// Render via the shared tabular text formatter.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// CLAN-compatible output matching legacy CLAN character-for-character.
    ///
    /// Format (from CLAN snapshot):
    /// ```text
    /// ----------------------------------------
    /// *** File "pipeout": line 10. Keyword: cookie
    /// *CHI:\tmore cookie . [+ IMP]
    /// ```
    fn render_clan(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        for m in &self.matches {
            writeln!(out, "----------------------------------------").ok();
            // CLAN uses "pipeout" as filename when reading from stdin pipe,
            // and 0-based line numbers (doesn't count the @UTF8 BOM line).
            writeln!(
                out,
                "*** File \"pipeout\": line {}. Keyword: {} ",
                m.line_number, m.keyword
            )
            .ok();
            writeln!(out, "{}", m.utterance_text).ok();
        }

        out
    }
}

/// Accumulated state for KWAL across all files.
#[derive(Debug, Default)]
pub struct KwalState {
    /// All matches found
    matches: Vec<KwalMatch>,
    /// Per-keyword match count
    keyword_counts: IndexMap<String, u64>,
}

/// KWAL command implementation.
///
/// For each utterance, extracts all countable words and checks whether
/// any match the configured keywords (case-insensitive). Matching
/// utterances are collected and displayed in the output.
#[derive(Debug, Clone, Default)]
pub struct KwalCommand {
    config: KwalConfig,
}

impl KwalCommand {
    /// Create a KWAL command with the given configuration.
    pub fn new(config: KwalConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for KwalCommand {
    type Config = KwalConfig;
    type State = KwalState;
    type Output = KwalResult;

    /// Find keyword matches in one utterance and record match metadata.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        if self.config.keywords.is_empty() {
            return;
        }

        // Collect normalized words from this utterance using the shared iterator
        let words: Vec<NormalizedWord> = countable_words(&utterance.main.content.content)
            .map(NormalizedWord::from_word)
            .collect();

        // Check which keywords match (case-insensitive, exact or wildcard)
        let mut matched = Vec::new();
        for keyword in &self.config.keywords {
            let kw_lower = keyword.to_lowercase();
            for word in &words {
                if word_pattern_matches(word.as_str(), &kw_lower) {
                    matched.push(keyword.clone());
                    break;
                }
            }
        }

        if !matched.is_empty() {
            // Record match counts
            for kw in &matched {
                *state.keyword_counts.entry(kw.to_string()).or_insert(0) += 1;
            }

            // Compute line number: O(log n) via LineMap when available, else 0
            let line_number = file_context
                .line_map
                .map(|lm| lm.line_of(utterance.main.span.start))
                .unwrap_or(0);

            // Serialize utterance text
            let utterance_text = utterance.main.to_chat_string();

            state.matches.push(KwalMatch {
                speaker: utterance.main.speaker.as_str().to_owned(),
                utterance_text,
                filename: file_context.filename.to_owned(),
                keyword: matched[0].to_string(),
                line_number,
            });
        }
    }

    /// Move collected match rows and keyword counters into typed output.
    fn finalize(&self, state: Self::State) -> KwalResult {
        KwalResult {
            matches: state.matches,
            keyword_counts: state.keyword_counts,
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

    /// Matching keywords should produce one row per matching utterance.
    #[test]
    fn kwal_finds_keyword() {
        let command = KwalCommand::new(KwalConfig {
            keywords: vec![crate::framework::KeywordPattern::from("cookie")],
        });
        let mut state = KwalState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utterance("CHI", &["I", "want", "cookie"]);
        let u2 = make_utterance("CHI", &["more", "milk"]);
        let u3 = make_utterance("MOT", &["have", "a", "cookie"]);

        command.process_utterance(&u1, &file_ctx, &mut state);
        command.process_utterance(&u2, &file_ctx, &mut state);
        command.process_utterance(&u3, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.keyword_counts["cookie"], 2);
    }

    /// Keyword matching should be case-insensitive.
    #[test]
    fn kwal_case_insensitive() {
        let command = KwalCommand::new(KwalConfig {
            keywords: vec![crate::framework::KeywordPattern::from("WANT")],
        });
        let mut state = KwalState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        assert_eq!(state.matches.len(), 1);
    }

    /// Exact keyword should NOT match partial words (CLAN parity).
    #[test]
    fn kwal_exact_match_no_substring() {
        let command = KwalCommand::new(KwalConfig {
            keywords: vec![crate::framework::KeywordPattern::from("cook")],
        });
        let mut state = KwalState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // "cook" does NOT match "cookie" without wildcard
        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        assert_eq!(state.matches.len(), 0);
    }

    /// Wildcard `*` should enable partial matching (CLAN parity).
    #[test]
    fn kwal_wildcard_match() {
        let command = KwalCommand::new(KwalConfig {
            keywords: vec![crate::framework::KeywordPattern::from("cook*")],
        });
        let mut state = KwalState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // "cook*" matches "cookie" via wildcard
        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        assert_eq!(state.matches.len(), 1);
    }

    /// The `word_pattern_matches` function should handle wildcards correctly.
    #[test]
    fn keyword_matches_patterns() {
        use crate::framework::word_pattern_matches;

        // Exact match
        assert!(word_pattern_matches("cookie", "cookie"));
        assert!(!word_pattern_matches("cookies", "cookie"));

        // Prefix wildcard
        assert!(word_pattern_matches("cookie", "cook*"));
        assert!(word_pattern_matches("cookies", "cook*"));
        assert!(!word_pattern_matches("book", "cook*"));

        // Suffix wildcard
        assert!(word_pattern_matches("going", "*ing"));
        assert!(!word_pattern_matches("gong", "*ing"));

        // Contains wildcard
        assert!(word_pattern_matches("cookie", "*oki*"));
        assert!(!word_pattern_matches("cook", "*oki*"));

        // Star alone matches everything
        assert!(word_pattern_matches("anything", "*"));
    }

    /// Non-matching keywords should leave output collections empty.
    #[test]
    fn kwal_no_matches() {
        let command = KwalCommand::new(KwalConfig {
            keywords: vec![crate::framework::KeywordPattern::from("zebra")],
        });
        let mut state = KwalState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u = make_utterance("CHI", &["I", "want", "cookie"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert!(result.matches.is_empty());
        assert!(result.keyword_counts.is_empty());
    }

    /// Empty keyword configuration should short-circuit to no matches.
    #[test]
    fn kwal_empty_keywords() {
        let command = KwalCommand::new(KwalConfig { keywords: vec![] });
        let mut state = KwalState::default();

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u = make_utterance("CHI", &["hello"]);
        command.process_utterance(&u, &file_ctx, &mut state);

        let result = command.finalize(state);
        assert!(result.matches.is_empty());
    }
}
