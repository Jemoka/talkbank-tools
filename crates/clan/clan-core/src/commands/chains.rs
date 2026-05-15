//! CHAINS — Clause chain analysis via code markers.
//!
//! Analyzes clause chains by tracking consecutive occurrences of codes
//! across utterances. A "chain" is a run of consecutive utterances (by the
//! same speaker) that all contain a given code on the `%cod` dependent tier.
//! When the code disappears or the speaker changes, the chain is flushed and
//! its length is recorded.
//!
//! Reports chain count, average/min/max length, and standard deviation per
//! code and speaker.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409147)
//! for the original CHAINS command specification.
//!
//! # Differences from CLAN
//!
//! - Speaker-change detection flushes all open chains for the previous speaker,
//!   matching CLAN's behavior of treating chains as speaker-scoped.
//! - Standard deviation uses the sample (N-1) formula rather than population.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
    cod_item_values, dependent_tier_content_text,
};

/// Configuration for the CHAINS command.
#[derive(Debug, Clone)]
pub struct ChainsConfig {
    /// Tier kind to read codes from (default: %cod).
    pub tier: crate::framework::TierKind,
}

impl Default for ChainsConfig {
    fn default() -> Self {
        Self {
            tier: crate::framework::TierKind::Cod,
        }
    }
}

/// Per-code chain statistics.
#[derive(Debug, Clone, Serialize)]
pub struct CodeChainStats {
    /// Code token.
    pub code: String,
    /// Number of separate chains.
    pub num_chains: u64,
    /// Average chain length.
    pub avg_length: f64,
    /// Standard deviation of chain lengths.
    pub std_dev: f64,
    /// Minimum chain length observed.
    pub min_length: u64,
    /// Maximum chain length observed.
    pub max_length: u64,
}

/// Per-speaker chain data.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerChains {
    /// Speaker identifier.
    pub speaker: String,
    /// Per-code chain statistics.
    pub codes: Vec<CodeChainStats>,
}

/// Typed output for the CHAINS command.
#[derive(Debug, Clone, Serialize)]
pub struct ChainsResult {
    /// Per-speaker chain data.
    pub speakers: Vec<SpeakerChains>,
}

impl ChainsResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("chains");
        for speaker in &self.speakers {
            let rows: Vec<TableRow> = speaker
                .codes
                .iter()
                .map(|c| TableRow {
                    values: vec![
                        c.code.clone(),
                        c.num_chains.to_string(),
                        format!("{:.2}", c.avg_length),
                        format!("{:.2}", c.std_dev),
                        c.min_length.to_string(),
                        c.max_length.to_string(),
                    ],
                })
                .collect();
            let section = Section::with_table(
                format!("Speaker: {}", speaker.speaker),
                vec![
                    "Code".to_owned(),
                    "# Chains".to_owned(),
                    "Avg Length".to_owned(),
                    "Std Dev".to_owned(),
                    "Min".to_owned(),
                    "Max".to_owned(),
                ],
                rows,
            );
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for ChainsResult {
    /// Render chain statistics as a human-readable text table.
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render chain statistics in CLAN-compatible format.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for speaker in &self.speakers {
            // CLAN skips speakers with no chains
            if speaker.codes.is_empty() {
                continue;
            }
            out.push_str(&format!("Speaker: {}\n", speaker.speaker));
            for c in &speaker.codes {
                out.push_str(&format!(
                    "  {:>10}  chains:{:>3}  avg:{:.2}  sd:{:.2}  min:{}  max:{}\n",
                    c.code, c.num_chains, c.avg_length, c.std_dev, c.min_length, c.max_length
                ));
            }
            out.push('\n');
        }
        out
    }
}

/// Tracking state for a single code's chains (internal).
///
/// Accumulates running statistics using Welford-style incremental
/// computation: sum and sum-of-squares are tracked so that mean and
/// sample standard deviation can be derived in `to_stats()` without
/// storing individual chain lengths.
#[derive(Debug, Default)]
struct ChainTracker {
    /// Number of completed chains.
    num_chains: u64,
    /// Sum of all chain lengths.
    total_length: f64,
    /// Sum of squared chain lengths.
    total_length_sq: f64,
    /// Minimum chain length.
    min_length: u64,
    /// Maximum chain length.
    max_length: u64,
    /// Current chain length.
    current: u64,
}

impl ChainTracker {
    /// Close the current chain (if any) and record its length in the
    /// running statistics. Resets `current` to zero.
    fn flush_chain(&mut self) {
        if self.current > 0 {
            self.num_chains += 1;
            self.total_length += self.current as f64;
            self.total_length_sq += (self.current as f64).powi(2);
            if self.min_length == 0 || self.current < self.min_length {
                self.min_length = self.current;
            }
            if self.current > self.max_length {
                self.max_length = self.current;
            }
            self.current = 0;
        }
    }

    /// Convert accumulated tracking data into a finalized [`CodeChainStats`].
    fn to_stats(&self, code: &str) -> CodeChainStats {
        let avg = if self.num_chains > 0 {
            self.total_length / self.num_chains as f64
        } else {
            0.0
        };
        let variance = if self.num_chains > 1 {
            (self.total_length_sq - self.total_length.powi(2) / self.num_chains as f64)
                / (self.num_chains as f64 - 1.0)
        } else {
            0.0
        };
        let std_dev = if variance > 0.0 { variance.sqrt() } else { 0.0 };
        CodeChainStats {
            code: code.to_owned(),
            num_chains: self.num_chains,
            avg_length: avg,
            std_dev,
            min_length: self.min_length,
            max_length: self.max_length,
        }
    }
}

/// Accumulated state for CHAINS across all files.
#[derive(Debug, Default)]
pub struct ChainsState {
    /// Speaker → (code → chain tracker).
    speakers: BTreeMap<String, BTreeMap<String, ChainTracker>>,
    /// Last speaker seen (for detecting speaker changes).
    last_speaker: Option<String>,
}

/// CHAINS command implementation.
///
/// Processes utterances sequentially, tracking which codes appear on the
/// `%cod` tier. Chains are flushed when a code disappears from consecutive
/// utterances or when the speaker changes.
pub struct ChainsCommand {
    /// Command configuration (tier label, etc.).
    config: ChainsConfig,
}

impl ChainsCommand {
    /// Create a new CHAINS command with the given config.
    pub fn new(config: ChainsConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for ChainsCommand {
    type Config = ChainsConfig;
    type State = ChainsState;
    type Output = ChainsResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();

        // On speaker change, flush all current chains for the previous speaker
        if state.last_speaker.as_ref() != Some(&speaker) {
            if let Some(prev_speaker) = &state.last_speaker
                && let Some(trackers) = state.speakers.get_mut(prev_speaker)
            {
                for tracker in trackers.values_mut() {
                    tracker.flush_chain();
                }
            }
            state.last_speaker = Some(speaker.clone());
        }

        // Extract codes from the specified tier
        let mut found_codes: Vec<String> = Vec::new();
        for dep in &utterance.dependent_tiers {
            if self.config.tier == dep.kind() {
                if let talkbank_model::DependentTier::Cod(tier) = dep {
                    found_codes.extend(cod_item_values(tier));
                } else {
                    found_codes.extend(
                        dependent_tier_content_text(dep)
                            .split_whitespace()
                            .filter(|token| !token.is_empty() && *token != ".")
                            .map(str::to_owned),
                    );
                }
            }
        }

        let speaker_trackers = state.speakers.entry(speaker).or_default();

        // Mark which codes are present in this utterance
        let present: std::collections::BTreeSet<&str> =
            found_codes.iter().map(|s| s.as_str()).collect();

        // For each code we're tracking, extend or flush chain
        let all_codes: Vec<String> = speaker_trackers.keys().cloned().collect();
        for code in &all_codes {
            if !present.contains(code.as_str())
                && let Some(tracker) = speaker_trackers.get_mut(code)
            {
                tracker.flush_chain();
            }
        }

        // Extend chains for codes present in this utterance
        for code in &found_codes {
            let tracker = speaker_trackers.entry(code.clone()).or_default();
            tracker.current += 1;
        }
    }

    fn finalize(&self, mut state: Self::State) -> ChainsResult {
        // Flush any remaining open chains
        for trackers in state.speakers.values_mut() {
            for tracker in trackers.values_mut() {
                tracker.flush_chain();
            }
        }

        let speakers = state
            .speakers
            .into_iter()
            .map(|(speaker, trackers)| {
                let codes: Vec<CodeChainStats> = trackers
                    .iter()
                    .map(|(code, tracker)| tracker.to_stats(code))
                    .collect();
                SpeakerChains { speaker, codes }
            })
            .collect();

        ChainsResult { speakers }
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
    fn chains_basic() {
        let cmd = ChainsCommand::new(ChainsConfig::default());
        let mut state = ChainsState::default();
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: &chat_file,
            filename: "test",
            line_map: None,
        };

        // Three consecutive utterances with code "A" = chain of length 3
        let u1 = make_utt_with_cod("CHI", "A");
        let u2 = make_utt_with_cod("CHI", "A");
        let u3 = make_utt_with_cod("CHI", "A");
        // Then break the chain
        let u4 = make_utt_with_cod("CHI", "B");

        cmd.process_utterance(&u1, &ctx, &mut state);
        cmd.process_utterance(&u2, &ctx, &mut state);
        cmd.process_utterance(&u3, &ctx, &mut state);
        cmd.process_utterance(&u4, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.speakers.len(), 1);
        let chi = &result.speakers[0];
        let a_stats = chi.codes.iter().find(|c| c.code == "A").unwrap();
        assert_eq!(a_stats.num_chains, 1);
        assert_eq!(a_stats.max_length, 3);
    }

    #[test]
    fn chains_empty() {
        let cmd = ChainsCommand::new(ChainsConfig::default());
        let state = ChainsState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }
}
