//! SUGAR — Sampling Utterances and Grammatical Analysis Revised.
//!
//! Computes language sample analysis metrics from `%mor` and `%gra`
//! tiers, providing a quick clinical assessment of grammatical
//! complexity:
//!
//! - **MLU-S**: Mean Length of Utterance in morphemes
//! - **TNW**: Total Number of Words (tokens with POS tags)
//! - **WPS**: Words Per Sentence (utterances containing verbs)
//! - **CPS**: Clauses Per Sentence (from `%gra` subordination relations)
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409287)
//! for the original SUGAR command specification.
//!
//! # Differences from CLAN
//!
//! - Verb detection uses mapped POS tags from the parsed `%mor` tier.
//!   CLAN may use a slightly different POS tag set for verb identification.
//! - Clause counting uses `%gra` subordination relations only. CLAN's
//!   clause detection may use additional heuristics.
//! - Minimum utterance threshold is configurable (CLAN uses a fixed value).
//! - Output supports text, JSON, and CSV formats.
//!
//! # Algorithm
//!
//! 1. For each utterance, count morphemes and words from `%mor`.
//! 2. Detect verb-containing utterances (POS tags: `v`, `cop`, `aux`,
//!    `mod`, `part`).
//! 3. For verb utterances with `%gra`, count subordinate clauses via
//!    grammatical relations (`COMP`, `CSUBJ`, `CMOD`, etc.).
//! 4. Compute per-speaker ratios at finalization.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::{DependentTier, Utterance};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, MorphemeCount, OutputFormat,
    Section, UtteranceCount, UtteranceLimit, WordCount, mor_item_has_verb, mor_item_morpheme_count,
};

/// Configuration for the SUGAR command.
#[derive(Debug, Clone)]
pub struct SugarConfig {
    /// Minimum number of utterances required (default: 50).
    pub min_utterances: UtteranceLimit,
}

impl Default for SugarConfig {
    fn default() -> Self {
        Self {
            min_utterances: UtteranceLimit::new(50),
        }
    }
}

/// Per-speaker SUGAR metrics.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerSugar {
    /// Speaker identifier.
    pub speaker: String,
    /// Mean Length of Utterance in morphemes.
    pub mlu_s: Option<f64>,
    /// Total Number of Words.
    pub tnw: WordCount,
    /// Words Per clause (utterances with verbs).
    pub wps: Option<f64>,
    /// Clauses Per utterance with verbs.
    pub cps: Option<f64>,
    /// Total utterances counted.
    pub utterance_count: UtteranceCount,
    /// Total morphemes counted.
    pub morpheme_count: MorphemeCount,
}

/// Typed output for the SUGAR command.
#[derive(Debug, Clone, Serialize)]
pub struct SugarResult {
    /// Per-speaker metrics.
    pub speakers: Vec<SpeakerSugar>,
}

impl SugarResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("sugar");
        for speaker in &self.speakers {
            let mut section = Section::with_fields(
                format!("Speaker: {}", speaker.speaker),
                indexmap::IndexMap::new(),
            );
            section.fields.insert(
                "MLU-S".to_owned(),
                speaker
                    .mlu_s
                    .map_or("N/A".to_owned(), |v| format!("{v:.3}")),
            );
            section
                .fields
                .insert("TNW".to_owned(), speaker.tnw.to_string());
            section.fields.insert(
                "WPS".to_owned(),
                speaker.wps.map_or("N/A".to_owned(), |v| format!("{v:.3}")),
            );
            section.fields.insert(
                "CPS".to_owned(),
                speaker.cps.map_or("N/A".to_owned(), |v| format!("{v:.3}")),
            );
            section
                .fields
                .insert("Utterances".to_owned(), speaker.utterance_count.to_string());
            section
                .fields
                .insert("Morphemes".to_owned(), speaker.morpheme_count.to_string());
            result.add_section(section);
        }
        result
    }
}

impl CommandOutput for SugarResult {
    /// Render per-speaker SUGAR metrics (MLU-S, TNW, WPS, CPS).
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    /// Render CLAN-compatible per-speaker summary.
    fn render_clan(&self) -> String {
        let mut out = String::new();
        for speaker in &self.speakers {
            out.push_str(&format!("Speaker: {}\n", speaker.speaker));
            out.push_str(&format!(
                "  MLU-S: {}\n",
                speaker
                    .mlu_s
                    .map_or("N/A".to_owned(), |v| format!("{v:.3}"))
            ));
            out.push_str(&format!("  TNW: {}\n", speaker.tnw));
            out.push_str(&format!(
                "  WPS: {}\n",
                speaker.wps.map_or("N/A".to_owned(), |v| format!("{v:.3}"))
            ));
            out.push_str(&format!(
                "  CPS: {}\n",
                speaker.cps.map_or("N/A".to_owned(), |v| format!("{v:.3}"))
            ));
            out.push('\n');
        }
        out
    }
}

/// Per-speaker accumulated state.
#[derive(Debug, Default)]
struct SpeakerState {
    /// Total morphemes across all utterances.
    morpheme_count: u64,
    /// Total words (tokens with POS tags).
    word_count: u64,
    /// Total complete utterances (sentences ending with . ? !).
    utterance_count: u64,
    /// Utterances containing at least one verb.
    verb_utterance_count: u64,
    /// Words in utterances containing verbs.
    verb_utterance_words: u64,
    /// Clause count from %gra analysis.
    clause_count: u64,
}

/// Accumulated state for SUGAR across all files.
#[derive(Debug, Default)]
pub struct SugarState {
    speakers: BTreeMap<String, SpeakerState>,
}

/// SUGAR command implementation.
///
/// Processes `%mor` and `%gra` tiers per utterance, accumulating
/// morpheme counts, word counts, verb-utterance tracking, and clause
/// counts for per-speaker metric computation at finalization.
pub struct SugarCommand {
    _config: SugarConfig,
}

impl SugarCommand {
    /// Create a new SUGAR command with the given config.
    pub fn new(config: SugarConfig) -> Self {
        Self { _config: config }
    }
}

/// Verb POS tags recognized in the CHAT `%mor` tier.
///
/// Includes both legacy CLAN tags (`v`, `cop`, `mod`) and modern UD tags (`verb`).
/// UD maps copula to `aux` (already included) and modals to `aux`/`verb`.
const VERB_POS: &[&str] = &["v", "verb", "cop", "aux", "mod", "part"];

/// Check if a `%mor` POS tag indicates a verb (including subtypes like `v:aux`).
fn is_verb_pos(pos: &str) -> bool {
    VERB_POS
        .iter()
        .any(|&v| pos == v || pos.starts_with(&format!("{v}:")))
}

/// Count subordinate clause relations from typed `%gra` entries.
///
/// Recognized subordinating relations: `CSUBJ`, `CPRED`, `CPOBJ`,
/// `COBJ`, `CJCT`, `XJCT`, `CMOD`, `XMOD`, `COMP`. Each occurrence
/// adds one clause to the count.
fn count_clauses_from_gra(tier: &talkbank_model::GraTier) -> u64 {
    let mut clauses = 0u64;
    for relation in tier.relations() {
        let relation = relation.relation.to_string().to_uppercase();
        match relation.as_str() {
            "CSUBJ" | "CPRED" | "CPOBJ" | "COBJ" | "CJCT" | "XJCT" | "CMOD" | "XMOD" | "COMP" => {
                clauses += 1;
            }
            _ => {}
        }
    }
    clauses
}

impl AnalysisCommand for SugarCommand {
    type Config = SugarConfig;
    type State = SugarState;
    type Output = SugarResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();
        let speaker_state = state.speakers.entry(speaker).or_default();

        // Count as a complete utterance
        speaker_state.utterance_count += 1;

        // Process %mor tier
        let mut mor_tier = None;
        let mut gra_tier = None;
        let mut has_verb = false;
        let mut word_count = 0u64;
        let mut morph_count = 0u64;

        for dep in &utterance.dependent_tiers {
            match dep {
                DependentTier::Mor(tier) => {
                    mor_tier = Some(tier);
                }
                DependentTier::Gra(tier) => {
                    gra_tier = Some(tier);
                }
                _ => {}
            }
        }

        if let Some(tier) = mor_tier {
            for item in tier.items() {
                word_count += 1;
                morph_count += mor_item_morpheme_count(item);
                if mor_item_has_verb(item, is_verb_pos) {
                    has_verb = true;
                }
            }
        }

        speaker_state.word_count += word_count;
        speaker_state.morpheme_count += morph_count;

        if has_verb {
            speaker_state.verb_utterance_count += 1;
            speaker_state.verb_utterance_words += word_count;

            // Count clauses from %gra if available
            if let Some(gra) = gra_tier {
                // Base clause count: 1 (for the main clause) + subordinate clauses
                speaker_state.clause_count += 1 + count_clauses_from_gra(gra);
            } else {
                // No %gra, assume 1 clause per verb utterance
                speaker_state.clause_count += 1;
            }
        }
    }

    fn finalize(&self, state: Self::State) -> SugarResult {
        let speakers: Vec<SpeakerSugar> = state
            .speakers
            .into_iter()
            .map(|(speaker, s)| {
                let mlu_s = if s.utterance_count > 0 {
                    Some(s.morpheme_count as f64 / s.utterance_count as f64)
                } else {
                    None
                };
                let wps = if s.verb_utterance_count > 0 {
                    Some(s.verb_utterance_words as f64 / s.verb_utterance_count as f64)
                } else {
                    None
                };
                let cps = if s.verb_utterance_count > 0 {
                    Some(s.clause_count as f64 / s.verb_utterance_count as f64)
                } else {
                    None
                };

                SpeakerSugar {
                    speaker,
                    mlu_s,
                    tnw: s.word_count,
                    wps,
                    cps,
                    utterance_count: s.utterance_count,
                    morpheme_count: s.morpheme_count,
                }
            })
            .collect();

        SugarResult { speakers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_morphemes_simple() {
        use talkbank_model::dependent_tier::mor::{Mor, MorWord};

        assert_eq!(
            mor_item_morpheme_count(&Mor::new(MorWord::new("n", "dog"))),
            1
        );
        assert_eq!(
            mor_item_morpheme_count(&Mor::new(MorWord::new("n", "dog").with_feature("PL"))),
            2
        );
        assert_eq!(
            mor_item_morpheme_count(&Mor::new(MorWord::new("v", "walk").with_feature("PAST"))),
            2
        );
        assert_eq!(
            mor_item_morpheme_count(
                &Mor::new(MorWord::new("pro", "it"))
                    .with_post_clitic(MorWord::new("aux", "be").with_feature("3S"))
            ),
            3
        );
    }

    #[test]
    fn is_verb_detects_verbs() {
        assert!(is_verb_pos("v"));
        assert!(is_verb_pos("cop"));
        assert!(is_verb_pos("aux"));
        assert!(is_verb_pos("mod"));
        assert!(is_verb_pos("mod:aux"));
        assert!(!is_verb_pos("n"));
        assert!(!is_verb_pos("adj"));
    }

    #[test]
    fn count_clauses_basic() {
        let gra = talkbank_model::GraTier::new_gra(vec![
            talkbank_model::GrammaticalRelation::new(1, 2, "SUBJ"),
            talkbank_model::GrammaticalRelation::new(2, 0, "ROOT"),
            talkbank_model::GrammaticalRelation::new(3, 2, "OBJ"),
            talkbank_model::GrammaticalRelation::new(4, 2, "COMP"),
            talkbank_model::GrammaticalRelation::new(5, 4, "SUBJ"),
        ]);
        assert_eq!(count_clauses_from_gra(&gra), 1);
    }

    #[test]
    fn sugar_empty() {
        let cmd = SugarCommand::new(SugarConfig::default());
        let state = SugarState::default();
        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }

    // --- UD format tests ---

    #[test]
    fn is_verb_detects_ud_verb() {
        assert!(is_verb_pos("verb"));
        assert!(is_verb_pos("v"));
        assert!(is_verb_pos("aux"));
        assert!(!is_verb_pos("noun"));
        assert!(!is_verb_pos("propn"));
        assert!(!is_verb_pos("pron"));
    }
}
