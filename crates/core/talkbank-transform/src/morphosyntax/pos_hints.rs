//! Transcriber-supplied `$POS` hint application and the Stanza
//! language-support gate.
//!
//! [`apply_pos_hints`] walks every utterance in a `ChatFile`, reads the
//! transcriber's `$POS` annotation off each Mor-domain word, and
//! overrides Stanza's UPOS in the corresponding `%mor` item when the
//! two disagree. The bookkeeping is summarized in [`HintOutcome`].
//! [`is_stanza_supported`] / [`supported_iso3_codes`] front the static
//! list of ISO-639-3 codes that Stanza actually has a pipeline for.

use std::collections::HashSet;
use std::sync::LazyLock;

use talkbank_model::alignment::helpers::{TierDomain, WordItem, walk_words};
use talkbank_model::model::Line;

use super::ud_types::UniversalPos;

/// Counts of hint-application outcomes across one `apply_pos_hints` run.
#[derive(Debug, Default, Clone, Copy)]
pub struct HintOutcome {
    /// Total `$POS`-annotated words encountered.
    pub hints_considered: usize,
    /// Hints where Stanza's POS already matched the transcriber's hint.
    pub hints_agreed: usize,
    /// Hints where Stanza's POS was replaced with the transcriber's hint.
    pub hints_overridden: usize,
    /// CLAN tags with no UD UPOS mapping.
    pub hints_unmapped: usize,
    /// Hints on utterances with no `%mor` tier to modify.
    pub hints_skipped_no_mor: usize,
}

/// Walk every utterance in `chat_file` and override `%mor` POS categories where
/// the transcriber's `$POS` hint disagrees with Stanza's output.
pub fn apply_pos_hints(chat_file: &mut talkbank_model::model::ChatFile) -> HintOutcome {
    use talkbank_model::model::content::word::Word;
    use talkbank_model::model::dependent_tier::mor::{MorTier, clan_to_ud_upos};
    use talkbank_model::model::{DependentTier, Utterance};

    fn collect_hints(line: &Line) -> Vec<(usize, String)> {
        let Line::Utterance(utt) = line else {
            return Vec::new();
        };
        let mut hints = Vec::new();
        let mut idx: usize = 0;
        walk_words(
            &utt.main.content.content,
            Some(TierDomain::Mor),
            &mut |leaf: WordItem| {
                let word: Option<&Word> = match leaf {
                    WordItem::Word(w) => Some(w),
                    WordItem::ReplacedWord(rw) => Some(&rw.word),
                    WordItem::Separator(_) => None,
                };
                if let Some(w) = word
                    && let Some(pos) = &w.part_of_speech
                {
                    hints.push((idx, pos.to_string()));
                }
                idx += 1;
            },
        );
        hints
    }

    fn mor_tier_mut(utt: &mut Utterance) -> Option<&mut MorTier> {
        utt.dependent_tiers.iter_mut().find_map(|t| match t {
            DependentTier::Mor(m) => Some(m),
            _ => None,
        })
    }

    enum HintResolution {
        Agreed,
        Overridden,
        Unmapped,
        NoMorItem,
    }

    fn resolve_hint(clan_tag: &str, mor: &mut MorTier, word_idx: usize) -> HintResolution {
        let Some(upos_name) = clan_to_ud_upos(clan_tag) else {
            return HintResolution::Unmapped;
        };
        let Some(hinted) = UniversalPos::from_pos_name(upos_name) else {
            return HintResolution::Unmapped;
        };
        let Some(mor_item) = mor.items_mut().get_mut(word_idx) else {
            return HintResolution::NoMorItem;
        };

        let stanza = UniversalPos::from_pos_name(mor_item.main.pos.as_ref());
        if stanza == Some(hinted) {
            return HintResolution::Agreed;
        }
        mor_item.override_main_pos(hinted.to_chat_pos_name());
        HintResolution::Overridden
    }

    let mut outcome = HintOutcome::default();

    for line_idx in 0..chat_file.lines.len() {
        let hints = collect_hints(&chat_file.lines[line_idx]);
        if hints.is_empty() {
            continue;
        }

        let utt = match &mut chat_file.lines[line_idx] {
            Line::Utterance(u) => u,
            _ => continue,
        };
        let Some(mor) = mor_tier_mut(utt) else {
            outcome.hints_considered += hints.len();
            outcome.hints_skipped_no_mor += hints.len();
            continue;
        };

        for (word_idx, clan_tag) in hints {
            outcome.hints_considered += 1;
            match resolve_hint(&clan_tag, mor, word_idx) {
                HintResolution::Agreed => outcome.hints_agreed += 1,
                HintResolution::Overridden => outcome.hints_overridden += 1,
                HintResolution::Unmapped => outcome.hints_unmapped += 1,
                HintResolution::NoMorItem => outcome.hints_skipped_no_mor += 1,
            }
        }
    }

    outcome
}

/// ISO 639-3 codes that have a known Stanza pipeline.
static SUPPORTED_STANZA_CODES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "eng", "spa", "fra", "deu", "ita", "por", "nld", "cat", "glg", "dan", "swe", "nor", "fin",
        "est", "lav", "lit", "isl", "pol", "ces", "ron", "hun", "bul", "hrv", "slk", "slv", "ukr",
        "rus", "ell", "cym", "gle", "gla", "eus", "mlt", "ara", "heb", "fas", "hin", "urd", "tur",
        "tam", "tel", "tha", "vie", "ind", "zho", "cmn", "yue", "jpn", "kor", "kat", "hye", "afr",
        "lat",
    ]
    .into_iter()
    .collect()
});

/// Check whether a language code is supported by the Stanza worker.
pub fn is_stanza_supported(lang: &talkbank_model::model::LanguageCode) -> bool {
    SUPPORTED_STANZA_CODES.contains(lang.as_ref())
}

/// Sorted list of ISO-639-3 codes the Rust gate considers Stanza-supported.
pub fn supported_iso3_codes() -> &'static [&'static str] {
    static SORTED: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
        let mut v: Vec<&'static str> = SUPPORTED_STANZA_CODES.iter().copied().collect();
        v.sort_unstable();
        v
    });
    &SORTED
}
