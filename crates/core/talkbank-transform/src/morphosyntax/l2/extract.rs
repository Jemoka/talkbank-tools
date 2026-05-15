//! Primary structural info extraction from UD responses.
//!
//! Extracts cross-linguistically valid structural information from the primary
//! model's UD output for @s word positions, before `inject_results` consumes
//! the data.

use talkbank_model::model::LanguageCode;
use talkbank_model::validation::LanguageResolution;

use super::deprel::UdDeprel;
use crate::morphosyntax::{BatchItemWithPosition, UdPunctable, UdResponse, UniversalPos};

/// Structural information from the primary model for one @s word.
///
/// This captures the cross-linguistically valid parts of the primary model's
/// analysis: dependency relation, head index, UPOS tag, and the dependency
/// relations of words that attach to this word (dependents).
#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryStructuralInfo {
    /// UD dependency relation from the primary model.
    pub deprel: UdDeprel,
    /// Universal POS tag from the primary model (may be wrong for OOV,
    /// but is structurally informed by context).
    pub upos: Option<UniversalPos>,
    /// Head word index (1-based UD convention; 0 = root).
    pub head: usize,
    /// Dependency relations of words that attach TO this word.
    pub dependent_deprels: Vec<UdDeprel>,
    /// UPOS of the head word (for GRA upgrade decisions).
    pub head_upos: Option<UniversalPos>,
}

impl PrimaryStructuralInfo {
    /// Whether any dependent has deprel "case" (oblique/prepositional phrase).
    pub fn has_case_dependent(&self) -> bool {
        self.dependent_deprels.iter().any(|d| d.base() == "case")
    }
}

/// A deferred @s position that needs secondary dispatch.
///
/// Created by `extract_l2_deferred_positions` before injection; consumed
/// by the secondary dispatch phase after injection.
#[derive(Debug, Clone, PartialEq)]
pub struct L2DeferredPosition {
    /// Line index in `ChatFile.lines`.
    pub line_idx: usize,
    /// Word index within the utterance (0-based, in MOR alignment domain).
    pub word_idx: usize,
    /// Resolved target language for secondary dispatch.
    pub target_lang: LanguageCode,
    /// Structural info from the primary model.
    pub primary: PrimaryStructuralInfo,
}

/// Resolve the dispatch target language from a `LanguageResolution`.
///
/// Uses `LanguageResolution::languages()` which returns the first language
/// for Single, all languages for Multiple/Ambiguous (we take the first),
/// and empty for Unresolved.
fn resolve_dispatch_lang(resolution: &LanguageResolution) -> Option<LanguageCode> {
    resolution.languages().first().cloned()
}

/// Extract deferred @s positions with their primary structural info.
///
/// This function reads (but does not consume) the primary model's UD responses
/// and the batch items' special_forms to identify which word positions need
/// secondary dispatch. Call this BEFORE `inject_results` so the UD data is
/// still available.
///
/// Words with `Unresolved` language resolution or no resolved dispatch target
/// are skipped — they will fall back to `L2|xxx` during injection.
pub fn extract_l2_deferred_positions(
    batch_items: &[BatchItemWithPosition],
    ud_responses: &[UdResponse],
) -> Vec<L2DeferredPosition> {
    let mut deferred = Vec::new();

    for (ud_resp, (line_idx, _utt_ordinal, item, _words)) in
        ud_responses.iter().zip(batch_items.iter())
    {
        let ud_sentence = match ud_resp.sentences.first() {
            Some(s) => s,
            None => continue,
        };

        for (word_idx, (_form_type, lang_res)) in item.special_forms.iter().enumerate() {
            let target_lang = match lang_res {
                Some(res) => match resolve_dispatch_lang(res) {
                    Some(lang) => lang,
                    None => continue,
                },
                None => continue,
            };

            let ud_word = ud_sentence.words.get(word_idx);

            let (deprel, upos, head) = match ud_word {
                Some(w) => {
                    let pos = match &w.upos {
                        UdPunctable::Value(p) => Some(*p),
                        UdPunctable::Punct(_) => None,
                    };
                    (UdDeprel::new(&w.deprel), pos, w.head)
                }
                None => (UdDeprel::new("dep"), None, 0),
            };

            let head_upos = if head > 0 {
                ud_sentence.words.get(head - 1).and_then(|w| match &w.upos {
                    UdPunctable::Value(p) => Some(*p),
                    UdPunctable::Punct(_) => None,
                })
            } else {
                None
            };

            let my_ud_idx = word_idx + 1;
            let dependent_deprels: Vec<UdDeprel> = ud_sentence
                .words
                .iter()
                .filter(|w| w.head == my_ud_idx)
                .map(|w| UdDeprel::new(&w.deprel))
                .collect();

            deferred.push(L2DeferredPosition {
                line_idx: *line_idx,
                word_idx,
                target_lang,
                primary: PrimaryStructuralInfo {
                    deprel,
                    upos,
                    head,
                    dependent_deprels,
                    head_upos,
                },
            });
        }
    }

    deferred
}
