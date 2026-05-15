//! MODREP — Model/replica comparison from `%mod` and `%pho` tiers.
//!
//! Compares the model (target) pronunciation on the `%mod` tier with the
//! actual (replica) pronunciation on the `%pho` tier, tracking word-by-word
//! mappings between model forms and replica forms. This is used in
//! phonological analysis to assess how closely a speaker's productions
//! match the adult target forms.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409226)
//! for the original MODREP command specification.
//!
//! # Algorithm
//!
//! 1. For each utterance with both `%mod` and `%pho` tiers:
//!    - Extract word lists from both tiers (flattening groups).
//!    - Pair words positionally (model word N <-> replica word N).
//!    - Record each (model, replica) pair in a frequency map per speaker.
//! 2. Report per-speaker tables of model words with their replica variants
//!    and frequency counts.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                                | Rust equivalent                           |
//! |---------------------------------------------|-------------------------------------------|
//! | `modrep +b%mod +c%pho file.cha`             | `chatter analyze modrep file.cha`         |
//! | `modrep +b%mod +c%pho +t*CHI file.cha`      | `chatter analyze modrep file.cha -s CHI`  |
//!
//! # Output
//!
//! Per-speaker listing of model words, each with its set of replica variants
//! and their frequency counts, sorted alphabetically by model word.
//!
//! # Differences from CLAN
//!
//! - Model and replica extraction uses parsed `%mod` and `%pho` tier
//!   structures from the AST rather than raw text line parsing.
//! - Word pairing operates on typed `PhoWord` content rather than
//!   string splitting.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::BTreeMap;
use std::fmt::Write;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::{PhoItem, PhoWord, SpeakerCode, Utterance};

use crate::framework::{AnalysisCommand, CommandOutput, FileContext};

/// Configuration for the MODREP command.
#[derive(Debug, Clone, Default)]
pub struct ModrepConfig {}

/// Accumulated replica variants for a single model word.
#[derive(Debug, Default)]
struct ModelWordData {
    /// Total occurrences of this model word.
    total: u64,
    /// Replica word → count (BTreeMap for alphabetical ordering).
    replicas: BTreeMap<String, u64>,
}

/// Per-speaker accumulated data.
#[derive(Debug, Default)]
struct SpeakerData {
    /// Model word → replica data (BTreeMap for alphabetical model word ordering).
    models: BTreeMap<String, ModelWordData>,
}

/// Accumulated state for MODREP across all files.
#[derive(Debug, Default)]
pub struct ModrepState {
    /// Per-speaker model/replica data, keyed by speaker code.
    by_speaker: IndexMap<SpeakerCode, SpeakerData>,
}

/// A single model→replica mapping entry.
#[derive(Debug, Clone, Serialize)]
pub struct ReplicaEntry {
    /// The replica word form.
    pub word: String,
    /// Number of times this model→replica pairing was observed.
    pub count: u64,
}

/// A single model word with its replica variants.
#[derive(Debug, Clone, Serialize)]
pub struct ModelEntry {
    /// The model (target) word form.
    pub model: String,
    /// Total occurrences of this model word.
    pub total: u64,
    /// All replica variants observed for this model word.
    pub replicas: Vec<ReplicaEntry>,
}

/// Per-speaker MODREP result.
#[derive(Debug, Clone, Serialize)]
pub struct ModrepSpeakerResult {
    /// Speaker code.
    pub speaker: String,
    /// Model word entries with replica variants, sorted alphabetically.
    pub entries: Vec<ModelEntry>,
}

/// Typed output from the MODREP command.
#[derive(Debug, Clone, Serialize)]
pub struct ModrepResult {
    /// Per-speaker results in encounter order.
    pub speakers: Vec<ModrepSpeakerResult>,
}

/// MODREP command: compare `%mod` and `%pho` tiers word-by-word.
///
/// Requires both tiers to be present on an utterance; utterances missing
/// either tier are silently skipped. When tiers have unequal lengths,
/// pairing stops at the shorter tier (`zip` truncation).
#[derive(Default)]
pub struct ModrepCommand;

impl ModrepCommand {
    /// Create a new `ModrepCommand` with the given configuration.
    pub fn new(_config: ModrepConfig) -> Self {
        Self
    }
}

impl AnalysisCommand for ModrepCommand {
    type Config = ModrepConfig;
    type State = ModrepState;
    type Output = ModrepResult;

    /// Compare aligned `%mod` and `%pho` token streams for one utterance.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Need both %mod and %pho tiers
        let mod_tier = match utterance.mod_tier() {
            Some(t) if t.is_mod() => t,
            _ => return,
        };
        let pho_tier = match utterance.pho_tier() {
            Some(t) if t.is_pho() => t,
            _ => return,
        };

        // Flatten both tiers into word lists
        let mod_words = flatten_pho_items(&mod_tier.items);
        let pho_words = flatten_pho_items(&pho_tier.items);

        let speaker_data = state
            .by_speaker
            .entry(utterance.main.speaker.clone())
            .or_default();

        // Pair words positionally (zip truncates to shorter tier)
        for (model_word, replica_word) in mod_words.iter().zip(pho_words.iter()) {
            let model_str = model_word.as_str().to_lowercase();
            let replica_str = replica_word.as_str().to_lowercase();

            let entry = speaker_data.models.entry(model_str).or_default();
            entry.total += 1;
            *entry.replicas.entry(replica_str).or_insert(0) += 1;
        }
    }

    /// Materialize sorted per-speaker model/replica frequency tables.
    fn finalize(&self, state: Self::State) -> Self::Output {
        let speakers = state
            .by_speaker
            .into_iter()
            .map(|(speaker_code, speaker_data)| {
                let entries = speaker_data
                    .models
                    .into_iter()
                    .map(|(model, data)| {
                        let replicas = data
                            .replicas
                            .into_iter()
                            .map(|(word, count)| ReplicaEntry { word, count })
                            .collect();
                        ModelEntry {
                            model,
                            total: data.total,
                            replicas,
                        }
                    })
                    .collect();
                ModrepSpeakerResult {
                    speaker: speaker_code.to_string(),
                    entries,
                }
            })
            .collect();

        ModrepResult { speakers }
    }
}

/// Flatten PhoItems into a simple list of PhoWords.
///
/// Groups are expanded into their constituent words.
///
/// # Postcondition
///
/// The returned list preserves original tier order after expanding groups.
fn flatten_pho_items(items: &[PhoItem]) -> Vec<&PhoWord> {
    let mut words = Vec::new();
    for item in items {
        match item {
            PhoItem::Word(word) => words.push(word),
            PhoItem::Group(group) => {
                for word in group.iter() {
                    words.push(word);
                }
            }
        }
    }
    words
}

impl CommandOutput for ModrepResult {
    /// Render per-speaker model/replica mappings in CLAN-like tabular text.
    fn render_text(&self) -> String {
        let mut out = String::new();

        for speaker in &self.speakers {
            writeln!(out, "Speaker *{}:", speaker.speaker).ok();
            for entry in &speaker.entries {
                writeln!(out, "  {:>3} {}", entry.total, entry.model).ok();
                for replica in &entry.replicas {
                    writeln!(out, "      {:>3} {}", replica.count, replica.word).ok();
                }
            }
            writeln!(out).ok();
        }

        out
    }

    /// CLAN output is currently identical to `render_text()` for this command.
    fn render_clan(&self) -> String {
        // CLAN format matches our text format for this command
        self.render_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{
        DependentTier, MainTier, PhoTier, Terminator, Utterance, UtteranceContent, Word,
    };

    /// Build an utterance with %mod and %pho tiers.
    fn make_mod_pho_utterance(
        words: &[&str],
        mod_tokens: &[&str],
        pho_tokens: &[&str],
    ) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let mut utt = Utterance::new(main);

        // Add %mod tier
        let mod_items: Vec<PhoItem> = mod_tokens
            .iter()
            .map(|t| PhoItem::Word(PhoWord::new(t.to_string())))
            .collect();
        utt.dependent_tiers
            .push(DependentTier::Mod(PhoTier::new_mod(mod_items)));

        // Add %pho tier
        let pho_items: Vec<PhoItem> = pho_tokens
            .iter()
            .map(|t| PhoItem::Word(PhoWord::new(t.to_string())))
            .collect();
        utt.dependent_tiers
            .push(DependentTier::Pho(PhoTier::new_pho(pho_items)));

        utt
    }

    /// Build a minimal FileContext for testing.
    fn make_file_context() -> FileContext<'static> {
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let leaked: &'static talkbank_model::ChatFile = Box::leak(Box::new(chat_file));
        FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: leaked,
            filename: "test",
            line_map: None,
        }
    }

    /// Matching `%mod`/`%pho` lengths should pair tokens positionally one-to-one.
    #[test]
    fn modrep_basic_pairing() {
        let cmd = ModrepCommand;
        let mut state = ModrepState::default();
        let utt = make_mod_pho_utterance(&["A", "B", "C"], &["d", "e", "f"], &["a", "b", "c"]);
        let file_ctx = make_file_context();

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        assert_eq!(result.speakers.len(), 1);
        let speaker = &result.speakers[0];
        assert_eq!(speaker.speaker, "CHI");
        assert_eq!(speaker.entries.len(), 3);

        // BTreeMap sorts alphabetically: d, e, f
        let d = &speaker.entries[0];
        assert_eq!(d.model, "d");
        assert_eq!(d.total, 1);
        assert_eq!(d.replicas.len(), 1);
        assert_eq!(d.replicas[0].word, "a");
        assert_eq!(d.replicas[0].count, 1);
    }

    /// Repeated model words should aggregate counts across multiple replica forms.
    #[test]
    fn modrep_accumulates_replicas() {
        let cmd = ModrepCommand;
        let mut state = ModrepState::default();
        let file_ctx = make_file_context();

        // Same model word "dog" with two different replicas
        let utt1 = make_mod_pho_utterance(&["dog"], &["dɔɡ"], &["dɔ"]);
        let utt2 = make_mod_pho_utterance(&["dog"], &["dɔɡ"], &["dɑɡ"]);
        let utt3 = make_mod_pho_utterance(&["dog"], &["dɔɡ"], &["dɔ"]);

        cmd.process_utterance(&utt1, &file_ctx, &mut state);
        cmd.process_utterance(&utt2, &file_ctx, &mut state);
        cmd.process_utterance(&utt3, &file_ctx, &mut state);

        let result = cmd.finalize(state);
        let speaker = &result.speakers[0];
        assert_eq!(speaker.entries.len(), 1);

        let dog = &speaker.entries[0];
        assert_eq!(dog.model, "dɔɡ");
        assert_eq!(dog.total, 3);
        assert_eq!(dog.replicas.len(), 2);

        // BTreeMap sorts alphabetically: dɑɡ before dɔ
        // (depends on Unicode ordering)
        let replica_counts: Vec<(&str, u64)> = dog
            .replicas
            .iter()
            .map(|r| (r.word.as_str(), r.count))
            .collect();
        assert!(replica_counts.contains(&("dɔ", 2)));
        assert!(replica_counts.contains(&("dɑɡ", 1)));
    }

    /// Utterances missing either `%mod` or `%pho` should be ignored.
    #[test]
    fn modrep_skips_without_both_tiers() {
        let cmd = ModrepCommand;
        let mut state = ModrepState::default();
        let file_ctx = make_file_context();

        // Utterance with only main tier — no %mod or %pho
        let content: Vec<UtteranceContent> =
            vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let utt = Utterance::new(main);

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        assert!(result.speakers.is_empty());
    }

    /// Pairing should stop at the shorter tier length (`zip` truncation).
    #[test]
    fn modrep_truncates_to_shorter_tier() {
        let cmd = ModrepCommand;
        let mut state = ModrepState::default();
        let file_ctx = make_file_context();

        // %mod has 3 words, %pho has 2 — should pair only 2
        let utt = make_mod_pho_utterance(&["A", "B", "C"], &["d", "e", "f"], &["a", "b"]);

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        let speaker = &result.speakers[0];
        assert_eq!(speaker.entries.len(), 2); // Only d→a and e→b, not f
    }

    /// Text rendering should include speaker header, model totals, and replica rows.
    #[test]
    fn modrep_render_text() {
        let result = ModrepResult {
            speakers: vec![ModrepSpeakerResult {
                speaker: "CHI".to_string(),
                entries: vec![ModelEntry {
                    model: "dɔɡ".to_string(),
                    total: 3,
                    replicas: vec![
                        ReplicaEntry {
                            word: "dɔ".to_string(),
                            count: 2,
                        },
                        ReplicaEntry {
                            word: "dɑɡ".to_string(),
                            count: 1,
                        },
                    ],
                }],
            }],
        };

        let text = result.render_text();
        assert!(text.contains("Speaker *CHI:"));
        assert!(text.contains("3 dɔɡ"));
        assert!(text.contains("2 dɔ"));
        assert!(text.contains("1 dɑɡ"));
    }
}
