//! PHONFREQ — Phonological frequency analysis from `%pho` tier.
//!
//! Counts individual phone (character) occurrences from `%pho` tier
//! content, tracking positional distribution within each phonological
//! word: initial (first character), final (last character), and other
//! (middle positions). All alphabetic characters (including IPA) and
//! compound markers (`+`) are counted, matching CLAN's behavior.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409227)
//! for the original PHONFREQ command specification.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                 | Rust equivalent                           |
//! |------------------------------|-------------------------------------------|
//! | `phonfreq file.cha`          | `chatter analyze phonfreq file.cha`       |
//! | `phonfreq +t*CHI file.cha`   | `chatter analyze phonfreq file.cha -s CHI`|
//!
//! # Output
//!
//! Per-phone frequency with positional breakdown (initial/final/other),
//! sorted alphabetically by phone character.
//!
//! # Differences from CLAN
//!
//! - Phone extraction uses parsed `%pho` tier structure from the AST
//!   rather than raw text character scanning.
//! - Positional classification operates on typed `PhoWord` content.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::BTreeMap;

use serde::Serialize;
use talkbank_model::{PhoItem, PhoWord, Utterance};

use crate::framework::{AnalysisCommand, CommandOutput, FileContext};

/// Configuration for the PHONFREQ command.
#[derive(Debug, Clone, Default)]
pub struct PhonfreqConfig {}

/// Positional counts for a single phone (character).
#[derive(Debug, Default)]
struct PhoneCounts {
    /// Total occurrences
    total: u64,
    /// Occurrences as first character of a pho word
    initial: u64,
    /// Occurrences as last character of a pho word
    final_pos: u64,
    /// Occurrences in middle positions
    other: u64,
}

/// Accumulated state for PHONFREQ across all files.
#[derive(Debug, Default)]
pub struct PhonfreqState {
    /// Phone counts (BTreeMap for alphabetical ordering by phone character)
    counts: BTreeMap<char, PhoneCounts>,
}

/// Typed output from the PHONFREQ command.
#[derive(Debug, Clone, Serialize)]
pub struct PhonfreqResult {
    /// Per-phone frequency entries, sorted alphabetically.
    pub entries: Vec<PhonfreqEntry>,
}

/// A single phone frequency entry.
#[derive(Debug, Clone, Serialize)]
pub struct PhonfreqEntry {
    /// The phone character
    pub phone: String,
    /// Total occurrences
    pub total: u64,
    /// Occurrences as first character of a pho word
    pub initial: u64,
    /// Occurrences as last character of a pho word
    pub final_pos: u64,
    /// Occurrences in middle positions
    pub other: u64,
}

/// PHONFREQ command: count phone frequencies from the `%pho` tier.
///
/// Iterates over `PhoItem`s (words and groups) on each utterance's
/// `%pho` tier, counting alphabetic characters (Unicode, including
/// IPA) plus the `+` compound marker — with positional tracking.
/// Stress marks (`ˈ`, `ˌ`), length marks (`ː`), digits, and other
/// non-letter symbols are skipped. Utterances without a `%pho`
/// tier are silently skipped.
pub struct PhonfreqCommand;

impl PhonfreqCommand {
    /// Create a new `PhonfreqCommand` with the given configuration.
    pub fn new(_config: PhonfreqConfig) -> Self {
        Self
    }
}

impl Default for PhonfreqCommand {
    /// Default command instance carries no runtime configuration.
    fn default() -> Self {
        Self
    }
}

impl AnalysisCommand for PhonfreqCommand {
    type Config = PhonfreqConfig;
    type State = PhonfreqState;
    type Output = PhonfreqResult;

    /// Count `%pho` character frequencies for one utterance when tier is present.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // Get %pho tier, skip utterances without one
        let pho_tier = match utterance.pho_tier() {
            Some(t) if t.is_pho() => t,
            _ => return,
        };

        for item in pho_tier.items.iter() {
            match item {
                PhoItem::Word(word) => {
                    count_pho_word(word, &mut state.counts);
                }
                PhoItem::Group(group) => {
                    for word in group.iter() {
                        count_pho_word(word, &mut state.counts);
                    }
                }
            }
        }
    }

    /// Convert accumulated phone maps into deterministic output rows.
    fn finalize(&self, state: Self::State) -> Self::Output {
        let entries: Vec<PhonfreqEntry> = state
            .counts
            .into_iter()
            .map(|(phone, counts)| PhonfreqEntry {
                phone: phone.to_string(),
                total: counts.total,
                initial: counts.initial,
                final_pos: counts.final_pos,
                other: counts.other,
            })
            .collect();

        PhonfreqResult { entries }
    }
}

/// Count each character in a phonological word, tracking position.
///
/// "Initial" = first character, "final" = last character, "other" = everything
/// in between. Single-character words count as both initial and final (matching
/// CLAN behavior where a single char has initial=1, final=0).
///
/// # Precondition
///
/// `word` should be a non-empty phonological transcription token.
fn count_pho_word(word: &PhoWord, counts: &mut BTreeMap<char, PhoneCounts>) {
    let text = word.as_str();
    if text.is_empty() {
        return;
    }

    // Count alphabetic characters (including IPA), plus `+` (compound
    // marker). Skip stress marks (ˈˌ), length marks (ː), digits, and
    // other non-letter symbols.
    let chars: Vec<char> = text
        .chars()
        .filter(|c| (c.is_alphabetic() || *c == '+') && !matches!(*c, 'ˈ' | 'ˌ' | 'ː'))
        .collect();
    let len = chars.len();
    if len == 0 {
        return;
    }

    for (i, &ch) in chars.iter().enumerate() {
        let entry = counts.entry(ch).or_default();
        entry.total += 1;

        if i == 0 {
            entry.initial += 1;
        } else if i == len - 1 {
            entry.final_pos += 1;
        } else {
            entry.other += 1;
        }
    }
}

impl CommandOutput for PhonfreqResult {
    /// Render per-phone totals and positional counts in CLAN-style columns.
    fn render_text(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        for entry in &self.entries {
            writeln!(
                out,
                "{:>3}  {:<4} initial = {:>3}, final = {:>3}, other = {:>3}",
                entry.total, entry.phone, entry.initial, entry.final_pos, entry.other,
            )
            .ok();
        }

        out
    }

    /// CLAN output currently matches `render_text()` exactly for this command.
    fn render_clan(&self) -> String {
        // CLAN format is identical to our text format for this command
        self.render_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{DependentTier, MainTier, PhoTier, Terminator, UtteranceContent, Word};

    /// Build an utterance with a %pho tier for testing.
    fn make_pho_utterance(words: &[&str], pho_tokens: &[&str]) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let mut utt = Utterance::new(main);

        let pho_items: Vec<PhoItem> = pho_tokens
            .iter()
            .map(|t| PhoItem::Word(PhoWord::new(t.to_string())))
            .collect();
        utt.dependent_tiers
            .push(DependentTier::Pho(PhoTier::new_pho(pho_items)));

        utt
    }

    /// Build a minimal FileContext for testing.
    fn make_file_context() -> (talkbank_model::ChatFile, FileContext<'static>) {
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        // Leak to get 'static reference — acceptable in test code
        let leaked: &'static talkbank_model::ChatFile = Box::leak(Box::new(chat_file));
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: leaked,
            filename: "test",
            line_map: None,
        };
        // Return the context (the ChatFile is leaked, no drop needed)
        (talkbank_model::ChatFile::new(vec![]), ctx)
    }

    /// A simple `%pho` token should emit one row per lowercase character.
    #[test]
    fn phonfreq_counts_characters() {
        let cmd = PhonfreqCommand;
        let mut state = PhonfreqState::default();
        let utt = make_pho_utterance(&["hello"], &["abc"]);
        let (_, file_ctx) = make_file_context();

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        assert_eq!(result.entries.len(), 3);

        let a = &result.entries[0];
        assert_eq!(a.phone, "a");
        assert_eq!(a.total, 1);
        assert_eq!(a.initial, 1);
        assert_eq!(a.final_pos, 0);
        assert_eq!(a.other, 0);

        let c = &result.entries[2];
        assert_eq!(c.phone, "c");
        assert_eq!(c.total, 1);
        assert_eq!(c.initial, 0);
        assert_eq!(c.final_pos, 1);
        assert_eq!(c.other, 0);
    }

    /// Repeated characters should accumulate initial/final/other buckets correctly.
    #[test]
    fn phonfreq_position_tracking() {
        let cmd = PhonfreqCommand;
        let mut state = PhonfreqState::default();
        let utt = make_pho_utterance(&["word"], &["abcba"]);
        let (_, file_ctx) = make_file_context();

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        // 'a': initial=1, final=1, other=0, total=2
        let a = result.entries.iter().find(|e| e.phone == "a").unwrap();
        assert_eq!(a.total, 2);
        assert_eq!(a.initial, 1);
        assert_eq!(a.final_pos, 1);
        assert_eq!(a.other, 0);

        // 'b': initial=0, final=0, other=2, total=2
        let b = result.entries.iter().find(|e| e.phone == "b").unwrap();
        assert_eq!(b.total, 2);
        assert_eq!(b.initial, 0);
        assert_eq!(b.final_pos, 0);
        assert_eq!(b.other, 2);

        // 'c': initial=0, final=0, other=1, total=1
        let c = result.entries.iter().find(|e| e.phone == "c").unwrap();
        assert_eq!(c.total, 1);
        assert_eq!(c.initial, 0);
        assert_eq!(c.final_pos, 0);
        assert_eq!(c.other, 1);
    }

    /// Utterances without `%pho` should not affect phone counts.
    #[test]
    fn phonfreq_skips_utterances_without_pho() {
        let cmd = PhonfreqCommand;
        let mut state = PhonfreqState::default();
        let content: Vec<UtteranceContent> =
            vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let utt = Utterance::new(main);
        let (_, file_ctx) = make_file_context();

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        assert!(result.entries.is_empty());
    }

    /// Counts should accumulate across multiple utterances in one state.
    #[test]
    fn phonfreq_accumulates_across_utterances() {
        let cmd = PhonfreqCommand;
        let mut state = PhonfreqState::default();
        let (_, file_ctx) = make_file_context();

        let utt1 = make_pho_utterance(&["one"], &["ab"]);
        let utt2 = make_pho_utterance(&["two"], &["ab"]);

        cmd.process_utterance(&utt1, &file_ctx, &mut state);
        cmd.process_utterance(&utt2, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        let a = result.entries.iter().find(|e| e.phone == "a").unwrap();
        assert_eq!(a.total, 2);
        assert_eq!(a.initial, 2);
    }

    /// Text rendering should expose all positional counters for each phone.
    #[test]
    fn phonfreq_render_text() {
        let result = PhonfreqResult {
            entries: vec![PhonfreqEntry {
                phone: "a".to_string(),
                total: 5,
                initial: 2,
                final_pos: 1,
                other: 2,
            }],
        };
        let text = result.render_text();
        assert!(text.contains("5"));
        assert!(text.contains("a"));
        assert!(text.contains("initial =   2"));
        assert!(text.contains("final =   1"));
        assert!(text.contains("other =   2"));
    }
}
