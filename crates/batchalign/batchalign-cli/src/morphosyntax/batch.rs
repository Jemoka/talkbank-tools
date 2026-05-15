//! Secondary L2 dispatch for @s words.

use std::collections::HashMap;

use super::worker::infer_batch;
use crate::chat_ops::morphosyntax_ops::BatchItemWithPosition;
use crate::chat_ops::morphosyntax_ops::l2;
use crate::chat_ops::{ChatFile, LanguageCode};
use crate::pipeline::PipelineServices;

fn secondary_dispatch_supported(
    registry: Option<&crate::stanza_registry::StanzaRegistry>,
    target_lang: &LanguageCode,
) -> bool {
    registry.is_some_and(|reg| reg.supports_morphosyntax(target_lang.as_ref()))
}

// ---------------------------------------------------------------------------
// Experimental: secondary L2 dispatch for @s words
// ---------------------------------------------------------------------------

/// Dispatch @s words to secondary language workers and splice results back.
///
/// This function:
/// 1. Groups deferred positions into contiguous spans by target language
/// 2. For each supported target language, builds a minimal `BatchItemWithPosition`
///    and dispatches it to a secondary Stanza worker via `infer_batch`
/// 3. Runs the structural merge algorithm (primary structural + secondary lexical)
/// 4. Splices merged morphology back into the ChatFile, replacing L2|xxx
/// 5. Falls back to L2|xxx for unsupported languages or dispatch failures
pub(crate) async fn dispatch_secondary_l2(
    chat_file: &mut ChatFile,
    deferred: &[l2::L2DeferredPosition],
    services: PipelineServices<'_>,
    filename: &str,
) {
    use crate::chat_ops::morphosyntax_ops::MorphosyntaxBatchItem;

    let dispatch_plan = l2::plan_secondary_dispatch(chat_file, deferred);

    // Group spans by target language for batched dispatch.
    let mut by_lang: HashMap<LanguageCode, Vec<&l2::L2SpanPlan>> = HashMap::new();
    for span in &dispatch_plan.spans {
        by_lang
            .entry(span.target_lang.clone())
            .or_default()
            .push(span);
    }

    tracing::info!(
        filename = %filename,
        deferred = deferred.len(),
        spans = dispatch_plan.spans.len(),
        languages = by_lang.len(),
        "L2 morphotag: dispatching @s words to secondary workers"
    );

    let mut merged_results: Vec<Option<l2::MergedL2Morphology>> = vec![None; deferred.len()];

    for (target_lang, lang_spans) in &by_lang {
        let lang3 = match crate::api::LanguageCode3::try_new(target_lang.as_ref()) {
            Ok(l) => l,
            Err(_) => {
                tracing::warn!(lang = %target_lang, "L2 morphotag: invalid language code");
                continue;
            }
        };

        let supported = secondary_dispatch_supported(services.pool.stanza_registry(), target_lang);

        if !supported {
            let total_words: usize = lang_spans.iter().map(|s| s.words.len()).sum();
            tracing::info!(
                lang = %lang3,
                words = total_words,
                registry_available = services.pool.stanza_registry().is_some(),
                "L2 morphotag: unsupported or unavailable language"
            );
            continue;
        }

        // Each span becomes one BatchItemWithPosition (one Stanza "sentence").
        let batch_items: Vec<BatchItemWithPosition> = lang_spans
            .iter()
            .map(|span| {
                let num_words = span.words.len();
                (
                    0, // line_idx placeholder
                    0, // utt_ordinal placeholder
                    MorphosyntaxBatchItem {
                        words: span.words.clone(),
                        terminator: talkbank_model::Terminator::Period {
                            span: talkbank_model::Span::DUMMY,
                        },
                        special_forms: vec![(None, None); num_words],
                        lang: target_lang.clone(),
                    },
                    Vec::new(), // no extracted words needed
                )
            })
            .collect();

        let empty_mwt: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();

        match infer_batch(services.pool, &batch_items, &lang3, &empty_mwt, true, None).await {
            Ok(responses) => {
                for (span, ud_resp) in lang_spans.iter().copied().zip(responses.iter()) {
                    if let Some(sentence) = ud_resp.sentences.first() {
                        match l2::merge_planned_secondary_span(span, deferred, sentence) {
                            Ok(merged_pairs) => {
                                for (global_idx, merged) in merged_pairs {
                                    merged_results[global_idx] = Some(merged);
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    lang = %lang3,
                                    error = %e,
                                    "L2 morphotag: planned secondary merge failed"
                                );
                            }
                        }
                    }
                }
                let total_words: usize = lang_spans.iter().map(|s| s.words.len()).sum();
                tracing::info!(
                    lang = %lang3,
                    spans = lang_spans.len(),
                    words = total_words,
                    "L2 morphotag: secondary dispatch succeeded"
                );
            }
            Err(e) => {
                tracing::warn!(lang = %lang3, error = %e, "L2 morphotag: secondary dispatch failed");
            }
        }
    }

    let outcome = l2::splice_l2_into_chat(chat_file, deferred, &merged_results);
    tracing::info!(
        filename = %filename,
        spliced = outcome.spliced,
        fallback = outcome.fallback,
        gra_upgraded = outcome.gra_upgraded,
        "L2 morphotag: splice complete"
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::stanza_registry::StanzaRegistry;
    use crate::types::worker::StanzaLanguageProcessors;

    use super::secondary_dispatch_supported;

    fn registry_with_caps() -> StanzaRegistry {
        let mut caps = BTreeMap::new();
        caps.insert(
            "eng".to_string(),
            StanzaLanguageProcessors {
                alpha2: "en".to_string(),
                processors: vec![
                    "tokenize".to_string(),
                    "pos".to_string(),
                    "lemma".to_string(),
                    "depparse".to_string(),
                ],
            },
        );
        caps.insert(
            "pan".to_string(),
            StanzaLanguageProcessors {
                alpha2: "pa".to_string(),
                processors: vec!["tokenize".to_string()],
            },
        );
        StanzaRegistry::from_capabilities(&caps)
    }

    #[test]
    fn secondary_dispatch_requires_runtime_registry() {
        let lang = crate::chat_ops::LanguageCode::new("eng");
        assert!(
            !secondary_dispatch_supported(None, &lang),
            "without runtime Stanza capabilities, L2 dispatch must skip conservatively"
        );
    }

    #[test]
    fn secondary_dispatch_rejects_partial_processor_language() {
        let registry = registry_with_caps();
        let lang = crate::chat_ops::LanguageCode::new("pan");
        assert!(
            !secondary_dispatch_supported(Some(&registry), &lang),
            "tokenize-only languages must not reach L2 morphotag worker bootstrap"
        );
    }

    #[test]
    fn secondary_dispatch_accepts_full_morphosyntax_language() {
        let registry = registry_with_caps();
        let lang = crate::chat_ops::LanguageCode::new("eng");
        assert!(secondary_dispatch_supported(Some(&registry), &lang));
    }
}
