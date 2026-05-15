//! Contiguous span grouping for secondary dispatch.
//!
//! Groups @s words into per-utterance contiguous spans by target language,
//! preserving within-span context for Stanza (e.g., "los niños" stays
//! together rather than being sent as two isolated words).

use std::collections::HashMap;

use talkbank_model::model::LanguageCode;
use talkbank_model::validation::LanguageResolution;

use super::extract::L2DeferredPosition;
use super::plan::plan_dispatch_spans;

/// A contiguous span of @s words sharing the same target language.
///
/// Used by `group_l2_spans` for per-utterance analysis of @s patterns.
#[derive(Debug, Clone, PartialEq)]
pub struct L2Span {
    /// Indices into the utterance's word list (0-based).
    pub word_indices: Vec<usize>,
    /// Resolved target language for secondary dispatch.
    pub target_lang: LanguageCode,
    /// Word texts extracted for the secondary batch. Each is a
    /// provenance-sealed `ChatCleanedText` derived from the upstream
    /// typed AST.
    pub words: Vec<talkbank_model::ChatCleanedText>,
}

/// A contiguous span of @s words ready for secondary dispatch, with
/// provenance tracking back to the deferred position array.
#[derive(Debug, Clone)]
pub struct DispatchSpan {
    /// Global indices into the `L2DeferredPosition` array.
    pub global_indices: Vec<usize>,
    /// Word texts for this span (sent as one sentence to Stanza). Each
    /// is a provenance-sealed `ChatCleanedText` derived from the
    /// upstream typed AST.
    pub words: Vec<talkbank_model::ChatCleanedText>,
    /// Target language for dispatch.
    pub target_lang: LanguageCode,
}

/// Resolve the dispatch target language from a `LanguageResolution`.
fn resolve_dispatch_lang(resolution: &LanguageResolution) -> Option<LanguageCode> {
    resolution.languages().first().cloned()
}

/// Group @s words into contiguous spans by target language.
///
/// Consecutive @s words with the same resolved target language are merged
/// into a single [`L2Span`]. Non-contiguous @s words or words with
/// different target languages produce separate spans.
///
/// Words with `Unresolved` language resolution are skipped (they will
/// fall back to `L2|xxx`).
pub fn group_l2_spans(
    special_forms: &[(
        Option<talkbank_model::model::FormType>,
        Option<LanguageResolution>,
    )],
    word_texts: &[talkbank_model::ChatCleanedText],
) -> Vec<L2Span> {
    let mut spans: Vec<L2Span> = Vec::new();

    for (idx, (_form_type, lang_res)) in special_forms.iter().enumerate() {
        let target_lang = match lang_res {
            Some(res) => match resolve_dispatch_lang(res) {
                Some(lang) => lang,
                None => continue,
            },
            None => continue,
        };

        let extends = spans.last().is_some_and(|last: &L2Span| {
            last.target_lang == target_lang
                && last
                    .word_indices
                    .last()
                    .is_some_and(|&prev| prev + 1 == idx)
        });

        if extends {
            if let Some(last) = spans.last_mut() {
                last.word_indices.push(idx);
                last.words.push(word_texts[idx].clone());
            }
        } else {
            spans.push(L2Span {
                word_indices: vec![idx],
                target_lang,
                words: vec![word_texts[idx].clone()],
            });
        }
    }

    spans
}

/// Group deferred positions into per-utterance contiguous spans for dispatch.
///
/// Within each utterance, consecutive @s words with the same target language
/// are merged into a single span. Each span becomes one sentence in the
/// secondary Stanza batch.
///
/// `word_cache` provides word texts keyed by `(line_idx, word_idx)`,
/// pre-extracted from the ChatFile.
pub fn group_deferred_into_dispatch_spans(
    deferred: &[L2DeferredPosition],
    word_cache: &HashMap<(usize, usize), talkbank_model::ChatCleanedText>,
) -> Vec<DispatchSpan> {
    plan_dispatch_spans(deferred, word_cache)
        .spans
        .into_iter()
        .map(|span| DispatchSpan {
            global_indices: span.deferred_indices,
            words: span.words,
            target_lang: span.target_lang,
        })
        .collect()
}
