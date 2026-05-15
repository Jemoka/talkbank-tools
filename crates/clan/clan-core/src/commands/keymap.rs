//! KEYMAP — Contingency tables for coded data.
//!
//! Builds contingency (co-occurrence) matrices showing how often one
//! behavioral code follows another across consecutive utterances. Given
//! a set of keyword codes, KEYMAP tracks each keyword occurrence on a
//! specified coding tier (default `%cod`) and records what codes appear
//! in the immediately following utterance, broken down by speaker.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409207)
//! for the original KEYMAP command specification.
//!
//! # Differences from CLAN
//!
//! - Code extraction uses parsed dependent tier content rather than raw text.
//! - Keyword matching is case-insensitive by default.
//! - Output supports text, JSON, and CSV formats.
//! - Deterministic ordering via `BTreeMap`.
//!
//! # Output
//!
//! Per speaker per keyword:
//! - Total keyword occurrences
//! - Following codes with speaker attribution and frequency counts

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    cod_item_values, dependent_tier_content_text,
};

/// Configuration for the KEYMAP command.
#[derive(Debug, Clone)]
pub struct KeymapConfig {
    /// Primary codes to track (keywords).
    pub keywords: Vec<crate::framework::KeywordPattern>,
    /// Tier kind to read codes from (default: %cod).
    pub tier: crate::framework::TierKind,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            tier: crate::framework::TierKind::Cod,
        }
    }
}

/// A single contingency entry: keyword followed by code.
#[derive(Debug, Clone, Serialize)]
pub struct ContingencyEntry {
    /// The keyword that was found.
    pub keyword: String,
    /// Speaker who produced the keyword.
    pub keyword_speaker: String,
    /// The following code.
    pub following_code: String,
    /// Speaker who produced the following code.
    pub following_speaker: String,
    /// Count of this specific transition.
    pub count: u64,
}

/// Per-speaker keyword occurrence data.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerKeywordData {
    /// Speaker identifier.
    pub speaker: String,
    /// Keyword.
    pub keyword: String,
    /// Total occurrences of this keyword.
    pub total: u64,
    /// Following code contingencies.
    pub following: Vec<FollowingCode>,
}

/// A following code and its count.
#[derive(Debug, Clone, Serialize)]
pub struct FollowingCode {
    /// Following speaker.
    pub speaker: String,
    /// Following code.
    pub code: String,
    /// Count.
    pub count: u64,
}

/// Typed output for the KEYMAP command.
#[derive(Debug, Clone, Serialize)]
pub struct KeymapResult {
    /// Per-speaker keyword data.
    pub data: Vec<SpeakerKeywordData>,
}

impl KeymapResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("keymap");
        for entry in &self.data {
            let rows: Vec<TableRow> = entry
                .following
                .iter()
                .map(|f| TableRow {
                    values: vec![f.speaker.clone(), f.code.clone(), f.count.to_string()],
                })
                .collect();
            let mut section = Section::with_table(
                format!("Speaker: {} — Keyword: {}", entry.speaker, entry.keyword),
                vec![
                    "Following Speaker".to_owned(),
                    "Following Code".to_owned(),
                    "Count".to_owned(),
                ],
                rows,
            );
            section
                .fields
                .insert("Total occurrences".to_owned(), entry.total.to_string());
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for KeymapResult {
    /// Render per-keyword contingency tables.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible keyword frequency output grouped by speaker.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for entry in &self.data {
            out.push_str(&format!(
                "Speaker {}:\n  Key word \"{}\" found {} times\n",
                entry.speaker, entry.keyword, entry.total
            ));
            // Group following by speaker
            let mut by_speaker: BTreeMap<&str, Vec<&FollowingCode>> = BTreeMap::new();
            for f in &entry.following {
                by_speaker.entry(f.speaker.as_str()).or_default().push(f);
            }
            for (sp, codes) in by_speaker {
                let total: u64 = codes.iter().map(|c| c.count).sum();
                out.push_str(&format!(
                    "    {} instances followed by speaker {}, of these\n",
                    total, sp
                ));
                for c in codes {
                    out.push_str(&format!(
                        "      code \"{}\" maps {} time{}\n",
                        c.code,
                        c.count,
                        if c.count == 1 { "" } else { "s" }
                    ));
                }
            }
        }
        out
    }
}

/// A code occurrence with its speaker context.
#[derive(Debug)]
struct CodeOccurrence {
    speaker: String,
    code: String,
}

/// Accumulated state for KEYMAP across all files.
#[derive(Debug, Default)]
pub struct KeymapState {
    /// Recent code occurrences (sliding window for following-code tracking).
    recent: Vec<CodeOccurrence>,
    /// Speaker → keyword → (following_speaker:following_code → count).
    contingencies: BTreeMap<String, BTreeMap<String, BTreeMap<String, u64>>>,
    /// Speaker → keyword → total count.
    keyword_counts: BTreeMap<String, BTreeMap<String, u64>>,
}

/// KEYMAP command implementation.
///
/// For each utterance, extracts codes from the configured tier and checks
/// whether the previous utterance contained a keyword code. If so, records
/// the (keyword, following-code) pair with speaker attribution. The most
/// recent utterance's codes are kept in a sliding window for next-utterance
/// matching.
pub struct KeymapCommand {
    config: KeymapConfig,
}

impl KeymapCommand {
    /// Create a new KEYMAP command with the given config.
    pub fn new(config: KeymapConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for KeymapCommand {
    type Config = KeymapConfig;
    type State = KeymapState;
    type Output = KeymapResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();

        // Extract codes from the specified tier
        let mut codes: Vec<String> = Vec::new();
        for dep in &utterance.dependent_tiers {
            if self.config.tier == dep.kind() {
                if let talkbank_model::DependentTier::Cod(tier) = dep {
                    codes.extend(cod_item_values(tier));
                } else {
                    codes.extend(
                        dependent_tier_content_text(dep)
                            .split_whitespace()
                            .filter(|token| !token.is_empty() && *token != ".")
                            .map(str::to_owned),
                    );
                }
            }
        }

        // For each code in this utterance
        for code in &codes {
            // Check if any recent occurrence was a keyword — if so, this is a following code
            for recent in &state.recent {
                if self
                    .config
                    .keywords
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(&recent.code))
                {
                    let key = format!("{}:{}", speaker, code);
                    *state
                        .contingencies
                        .entry(recent.speaker.clone())
                        .or_default()
                        .entry(recent.code.clone())
                        .or_default()
                        .entry(key)
                        .or_insert(0) += 1;
                }
            }

            // Track if this code is a keyword
            if self
                .config
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case(code))
            {
                *state
                    .keyword_counts
                    .entry(speaker.clone())
                    .or_default()
                    .entry(code.clone())
                    .or_insert(0) += 1;
            }
        }

        // Update recent codes (keep only codes from this utterance for next-utterance matching)
        state.recent.clear();
        for code in codes {
            state.recent.push(CodeOccurrence {
                speaker: speaker.clone(),
                code,
            });
        }
    }

    fn finalize(&self, state: Self::State) -> KeymapResult {
        let mut data = Vec::new();

        for (speaker, keywords) in &state.keyword_counts {
            for (keyword, total) in keywords {
                let mut following = Vec::new();

                if let Some(contingency) = state
                    .contingencies
                    .get(speaker)
                    .and_then(|kw| kw.get(keyword))
                {
                    for (key, count) in contingency {
                        let parts: Vec<&str> = key.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            following.push(FollowingCode {
                                speaker: parts[0].to_owned(),
                                code: parts[1].to_owned(),
                                count: *count,
                            });
                        }
                    }
                }

                data.push(SpeakerKeywordData {
                    speaker: speaker.clone(),
                    keyword: keyword.clone(),
                    total: *total,
                    following,
                });
            }
        }

        KeymapResult { data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{CodTier, DependentTier, MainTier, Terminator, UtteranceContent, Word};

    fn make_utt_with_cod(speaker: &str, cod_text: &str) -> Utterance {
        let content = vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        let mut utt = Utterance::new(main);
        utt.dependent_tiers
            .push(DependentTier::Cod(CodTier::from_text(cod_text)));
        utt
    }

    #[test]
    fn keymap_basic() {
        let cmd = KeymapCommand::new(KeymapConfig {
            keywords: vec![crate::framework::KeywordPattern::from("A")],
            tier: crate::framework::TierKind::Cod,
        });
        let mut state = KeymapState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // Keyword "A" followed by "B" in next utterance
        let u1 = make_utt_with_cod("CHI", "A");
        let u2 = make_utt_with_cod("MOT", "B");

        cmd.process_utterance(&u1, &ctx, &mut state);
        cmd.process_utterance(&u2, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.data.len(), 1);
        assert_eq!(result.data[0].keyword, "A");
        assert_eq!(result.data[0].total, 1);
    }

    #[test]
    fn keymap_does_not_treat_selectors_as_keywords() {
        let cmd = KeymapCommand::new(KeymapConfig {
            keywords: vec![crate::framework::KeywordPattern::from("$WR")],
            tier: crate::framework::TierKind::Cod,
        });
        let mut state = KeymapState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        let u1 = make_utt_with_cod("CHI", "<w4> $WR");
        let u2 = make_utt_with_cod("MOT", "B");

        cmd.process_utterance(&u1, &ctx, &mut state);
        cmd.process_utterance(&u2, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.data.len(), 1);
        assert_eq!(result.data[0].keyword, "$WR");
        assert_eq!(result.data[0].total, 1);
        assert_eq!(result.data[0].following[0].code, "B");
    }

    #[test]
    fn keymap_empty() {
        let cmd = KeymapCommand::new(KeymapConfig::default());
        let state = KeymapState::default();
        let result = cmd.finalize(state);
        assert!(result.data.is_empty());
    }
}
