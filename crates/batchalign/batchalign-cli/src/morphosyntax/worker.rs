//! Worker dispatch for morphosyntax inference.

use std::collections::HashMap;
use std::sync::Arc;

use crate::api::LanguageCode3;
use crate::chat_ops::morphosyntax_ops::{BatchItemWithPosition, MwtDict};
use crate::chat_ops::nlp::UdResponse;
use crate::error::ServerError;
use crate::infer_retry::dispatch_execute_v2_with_retry_and_progress;
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::pool::WorkerPool;
use crate::worker::text_request_v2::{PreparedTextRequestIdsV2, build_morphosyntax_request_v2};
use crate::worker::text_result_v2::parse_morphosyntax_result_v2;
use talkbank_transform::morphosyntax::{diagnose_parse_failure, parse_raw_stanza_output};
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LanguageBatchGroup {
    lang: LanguageCode3,
    indices: Vec<usize>,
}

fn language_groups_for_items(
    items: &[BatchItemWithPosition],
    fallback_lang: &LanguageCode3,
) -> Result<Vec<LanguageBatchGroup>, ServerError> {
    let mut groups: Vec<LanguageBatchGroup> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();

    for (idx, (_, _, item, _)) in items.iter().enumerate() {
        let effective_lang = if item.lang.as_ref().is_empty() {
            fallback_lang.clone()
        } else {
            LanguageCode3::try_new(item.lang.as_ref()).map_err(|error| {
                ServerError::Validation(format!(
                    "morphotag batch item has invalid language '{}': {error}",
                    item.lang.as_ref()
                ))
            })?
        };

        let key = effective_lang.as_ref().to_string();
        if let Some(group_idx) = positions.get(&key).copied() {
            groups[group_idx].indices.push(idx);
        } else {
            positions.insert(key, groups.len());
            groups.push(LanguageBatchGroup {
                lang: effective_lang,
                indices: vec![idx],
            });
        }
    }

    Ok(groups)
}

/// Send batch items to workers for NLP inference via batched `execute_v2`.
///
/// When the batch is large enough and the pool allows multiple workers per
/// language key, the items are split into chunks and dispatched concurrently
/// to separate workers.  This is transparent to callers — the returned
/// `Vec<UdResponse>` is always parallel to the input `items` slice.
pub(crate) async fn infer_batch(
    pool: &WorkerPool,
    items: &[BatchItemWithPosition],
    lang: &LanguageCode3,
    mwt: &MwtDict,
    retokenize: bool,
    progress_tx: Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
) -> Result<Vec<UdResponse>, ServerError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let groups = language_groups_for_items(items, lang)?;
    let (dispatchable, fallback) =
        partition_groups_by_stanza_support(groups, pool.stanza_registry());

    let needs_grouping = dispatchable.len() > 1
        || dispatchable.first().map(|g| &g.lang) != Some(lang)
        || !fallback.is_empty();

    if !needs_grouping {
        // Single homogeneous supported group matching the caller's
        // fallback lang — the simple fast path.
        return infer_batch_homogeneous(pool, items, lang, mwt, retokenize, progress_tx).await;
    }

    info!(
        items = items.len(),
        dispatched_groups = dispatchable.len(),
        unsupported_groups = fallback.len(),
        "Dispatching mixed-language morphosyntax batch by per-item language; \
         items in unsupported languages get empty responses (BA2-equivalent L2|xxx fallback)"
    );

    let mut merged: Vec<Option<UdResponse>> = vec![None; items.len()];

    // Fill items in unsupported-language groups with empty UdResponses.
    // Downstream `inject_results` skips items whose response has no
    // sentences, leaving the existing `L2|xxx` placeholder in `%mor` —
    // matching the pre-BA3 fallback semantics for code-switches into
    // languages Stanza cannot analyze.
    for (group_lang, indices) in fallback {
        info!(
            lang = %group_lang,
            items = indices.len(),
            "Stanza does not support this language; emitting L2|xxx fallback for these items"
        );
        for idx in indices {
            merged[idx] = Some(UdResponse {
                sentences: Vec::new(),
            });
        }
    }

    for group in dispatchable {
        let group_items: Vec<BatchItemWithPosition> = group
            .indices
            .iter()
            .map(|&idx| items[idx].clone())
            .collect();
        let responses = infer_batch_homogeneous(
            pool,
            &group_items,
            &group.lang,
            mwt,
            retokenize,
            progress_tx,
        )
        .await?;
        for (original_idx, response) in group.indices.into_iter().zip(responses.into_iter()) {
            merged[original_idx] = Some(response);
        }
    }

    merged
        .into_iter()
        .map(|response| {
            response.ok_or_else(|| {
                ServerError::Validation(
                    "morphotag mixed-language dispatch returned incomplete results".into(),
                )
            })
        })
        .collect()
}

/// Split language groups into ones that should be dispatched to a
/// Stanza worker and ones that should fall back to `L2|xxx` because
/// Stanza lacks core morphosyntax processors for the language.
///
/// Returns `(dispatchable, fallback)` where:
///   - `dispatchable` is groups whose lang the registry supports;
///     these get sent to workers as before.
///   - `fallback` is `(lang, indices)` pairs for unsupported groups;
///     callers fill the corresponding response slots with empty
///     `UdResponse { sentences: vec![] }` so downstream injection
///     skips them and leaves the `L2|xxx` placeholder intact.
///
/// This lets a transcript declare unsupported secondary languages
/// (e.g. `@Languages: cym, eng, nep`) without crashing the worker
/// during bootstrap. Only utterances that actually code-switch into
/// the unsupported language fall back to `L2|xxx`; the rest are
/// dispatched normally.
fn partition_groups_by_stanza_support(
    groups: Vec<LanguageBatchGroup>,
    registry: Option<&crate::stanza_registry::StanzaRegistry>,
) -> (Vec<LanguageBatchGroup>, Vec<(LanguageCode3, Vec<usize>)>) {
    let mut dispatchable = Vec::new();
    let mut fallback = Vec::new();
    for group in groups {
        let supported = registry.is_some_and(|r| r.supports_morphosyntax(group.lang.as_ref()));
        if supported {
            dispatchable.push(group);
        } else {
            fallback.push((group.lang, group.indices));
        }
    }
    (dispatchable, fallback)
}

async fn infer_batch_homogeneous(
    pool: &WorkerPool,
    items: &[BatchItemWithPosition],
    lang: &LanguageCode3,
    mwt: &MwtDict,
    retokenize: bool,
    progress_tx: Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
) -> Result<Vec<UdResponse>, ServerError> {
    // Python workers emit `stage="stanza_processing"` on every progress
    // event (see `python/batchalign/worker/_protocol.py::write_progress_event`).
    // The drain loop in `runner/dispatch/infer_batched.rs` keys
    // `BatchInferProgress` by `event.stage`, so without a language-aware
    // rewrite every language collapses into one bucket and stall
    // detection goes blind. The tagger below rewrites `event.stage` to
    // the language code for the duration of this batch. Only spawned
    // when the caller actually wants progress updates.
    let tagger = ProgressTagger::install(progress_tx, lang);
    let inner_tx = tagger.sender();

    let num_chunks = compute_chunk_count(
        items.len(),
        pool.max_workers_per_key_for(crate::worker::WorkerProfile::Stanza),
    );

    let result = if num_chunks <= 1 {
        infer_batch_single(pool, items, lang, mwt, retokenize, inner_tx).await
    } else {
        let chunk_size = items.len().div_ceil(num_chunks);
        let chunks: Vec<&[BatchItemWithPosition]> = items.chunks(chunk_size).collect();
        info!(
            items = items.len(),
            chunks = chunks.len(),
            chunk_size,
            lang = %lang,
            "Splitting morphosyntax batch across workers"
        );
        let futures: Vec<_> = chunks
            .iter()
            .map(|chunk| infer_batch_single(pool, chunk, lang, mwt, retokenize, inner_tx))
            .collect();
        let outcomes = futures::future::join_all(futures).await;
        let mut all = Vec::with_capacity(items.len());
        for outcome in outcomes {
            all.extend(outcome?);
        }
        Ok(all)
    };

    tagger.close().await;
    result
}

/// Owns the inner mpsc channel + forwarder task that rewrites
/// `event.stage` on every progress event. Explicit struct so the
/// ownership boundary is visible (inner channel lives exactly for the
/// duration of a batch; outer channel is borrowed from the caller).
struct ProgressTagger {
    inner_tx: Option<tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl ProgressTagger {
    fn install(
        outer: Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
        lang: &LanguageCode3,
    ) -> Self {
        let outer = match outer {
            Some(tx) => tx.clone(),
            None => {
                return Self {
                    inner_tx: None,
                    handle: None,
                };
            }
        };
        let (inner_tx, mut inner_rx) =
            tokio::sync::mpsc::channel::<crate::types::worker_v2::ProgressEventV2>(64);
        let tag: Arc<str> = Arc::from(lang.as_ref());
        let handle = tokio::spawn(async move {
            while let Some(mut event) = inner_rx.recv().await {
                event.stage = tag.as_ref().to_string();
                if outer.send(event).await.is_err() {
                    break;
                }
            }
        });
        Self {
            inner_tx: Some(inner_tx),
            handle: Some(handle),
        }
    }

    fn sender(
        &self,
    ) -> Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>> {
        self.inner_tx.as_ref()
    }

    async fn close(self) {
        drop(self.inner_tx);
        if let Some(handle) = self.handle {
            let _ = handle.await;
        }
    }
}

/// Dispatch a single chunk of batch items to one worker.
///
/// This is the original `infer_batch` body, extracted so it can be called
/// once (fast path) or N times concurrently (chunked path).
async fn infer_batch_single(
    pool: &WorkerPool,
    items: &[BatchItemWithPosition],
    lang: &LanguageCode3,
    mwt: &MwtDict,
    retokenize: bool,
    progress_tx: Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
) -> Result<Vec<UdResponse>, ServerError> {
    let payload_items: Vec<_> = items.iter().map(|(_, _, item, _)| item.clone()).collect();

    let artifacts = PreparedArtifactRuntimeV2::new("morphosyntax_v2").map_err(|error| {
        ServerError::Validation(format!(
            "failed to create morphosyntax V2 artifact runtime: {error}"
        ))
    })?;
    let request_ids = PreparedTextRequestIdsV2::for_task("morphosyntax");
    let request = build_morphosyntax_request_v2(
        artifacts.store(),
        &request_ids,
        lang,
        &payload_items,
        mwt,
        retokenize,
    )
    .map_err(|error| {
        ServerError::Validation(format!(
            "failed to build morphosyntax V2 worker request: {error}"
        ))
    })?;

    info!(
        num_items = items.len(),
        lang = %lang,
        "Dispatching morphosyntax execute_v2 batch"
    );

    let response =
        dispatch_execute_v2_with_retry_and_progress(pool, lang, &request, progress_tx).await?;
    let result = parse_morphosyntax_result_v2(&response).map_err(|error| {
        ServerError::Validation(format!("invalid morphosyntax V2 result: {error}"))
    })?;
    if result.items.len() != items.len() {
        return Err(ServerError::Validation(format!(
            "morphosyntax V2 returned {} items for {} requests",
            result.items.len(),
            items.len()
        )));
    }

    let mut ud_responses = Vec::with_capacity(result.items.len());
    for (i, item_result) in result.items.iter().enumerate() {
        if let Some(error) = &item_result.error {
            warn!(item = i, error = %error, "Infer error for item (using empty response)");
            ud_responses.push(UdResponse {
                sentences: Vec::new(),
            });
            continue;
        }

        if let Some(raw_sentences) = &item_result.raw_sentences {
            let ud = parse_raw_stanza_output(raw_sentences).map_err(|error| {
                // Include the words sent, structured diagnostics, and raw
                // Stanza output so the failure is diagnosable without replay.
                let words_sent = &payload_items[i].words;
                let diagnostics = diagnose_parse_failure(raw_sentences);
                let diag_str = if diagnostics.is_empty() {
                    "no structural issues detected by diagnostics".to_string()
                } else {
                    diagnostics
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                let raw_json = serde_json::to_string(raw_sentences)
                    .unwrap_or_else(|_| "<serialization failed>".into());
                warn!(
                    item = i,
                    words = ?words_sent,
                    diagnostics = %diag_str,
                    raw_stanza_output = %raw_json,
                    %error,
                    "Stanza output parse failure — full diagnostics logged"
                );
                ServerError::Validation(format!(
                    "Failed to parse raw Stanza output for item {i} \
                     (words: {words_sent:?}): {error}. Diagnostics: {diag_str}"
                ))
            })?;
            ud_responses.push(ud);
            continue;
        }

        warn!(
            item = i,
            "Morphosyntax V2 returned no raw_sentences and no error (using empty response)"
        );
        ud_responses.push(UdResponse {
            sentences: Vec::new(),
        });
    }

    Ok(ud_responses)
}

/// Minimum items per chunk.  Below this threshold, Stanza's per-batch
/// overhead (model forward-pass setup, tokenizer warmup) dominates and
/// splitting provides no throughput benefit.
const MIN_CHUNK_SIZE: usize = 30;

/// Compute how many worker chunks to split a language batch into.
///
/// Returns 1 (no split) when:
/// - Fewer than `MIN_CHUNK_SIZE` items (splitting not worthwhile).
/// - `max_workers` is 1 (only one worker slot available).
///
/// Otherwise returns `min(item_count / MIN_CHUNK_SIZE, max_workers)`.
fn compute_chunk_count(item_count: usize, max_workers: usize) -> usize {
    if item_count < MIN_CHUNK_SIZE || max_workers <= 1 {
        return 1;
    }
    (item_count / MIN_CHUNK_SIZE).clamp(1, max_workers)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_ops::morphosyntax_ops::MorphosyntaxBatchItem;
    use talkbank_model::Span;
    use talkbank_model::Terminator;

    fn batch_item(lang: &str) -> BatchItemWithPosition {
        (
            0,
            0,
            MorphosyntaxBatchItem {
                words: Vec::new(),
                terminator: Terminator::Period { span: Span::DUMMY },
                special_forms: Vec::new(),
                lang: talkbank_model::model::LanguageCode::new(lang),
            },
            Vec::new(),
        )
    }

    #[test]
    fn compute_chunk_count_below_minimum_returns_one() {
        assert_eq!(compute_chunk_count(0, 4), 1);
        assert_eq!(compute_chunk_count(1, 4), 1);
        assert_eq!(compute_chunk_count(29, 4), 1);
    }

    #[test]
    fn compute_chunk_count_at_minimum_returns_one() {
        // 30 / 30 = 1
        assert_eq!(compute_chunk_count(30, 4), 1);
    }

    #[test]
    fn compute_chunk_count_scales_with_items() {
        assert_eq!(compute_chunk_count(60, 4), 2);
        assert_eq!(compute_chunk_count(90, 4), 3);
        assert_eq!(compute_chunk_count(120, 4), 4);
    }

    #[test]
    fn compute_chunk_count_clamped_by_max_workers() {
        // 2000 / 30 = 66, but max_workers = 4
        assert_eq!(compute_chunk_count(2000, 4), 4);
        assert_eq!(compute_chunk_count(500, 2), 2);
        assert_eq!(compute_chunk_count(500, 8), 8);
    }

    #[test]
    fn compute_chunk_count_single_worker_always_one() {
        assert_eq!(compute_chunk_count(2000, 1), 1);
        assert_eq!(compute_chunk_count(60, 1), 1);
    }

    #[test]
    fn compute_chunk_count_zero_workers_returns_one() {
        // Defensive: max_workers=0 should not panic
        assert_eq!(compute_chunk_count(100, 0), 1);
    }

    #[test]
    fn language_groups_for_items_uses_per_item_language_and_preserves_indices() {
        let items = vec![batch_item("eng"), batch_item("spa"), batch_item("eng")];
        let groups = language_groups_for_items(&items, &LanguageCode3::eng()).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].lang, LanguageCode3::eng());
        assert_eq!(groups[0].indices, vec![0, 2]);
        assert_eq!(groups[1].lang.as_ref(), "spa");
        assert_eq!(groups[1].indices, vec![1]);
    }

    /// Regression test for the worker-bootstrap crash on unsupported
    /// secondary languages (e.g. `@Languages: cym, eng, nep` —
    /// pre-fix, the morphotag pipeline tried to spawn a Stanza nep
    /// worker for `[- nep]`-precoded utterances and crashed during
    /// bootstrap with `UnsupportedLanguageError`, leaving the file's
    /// processing as "failed to parse ready signal").
    ///
    /// The new partition function splits groups by Stanza support so
    /// unsupported-language items can be filled with empty
    /// `UdResponse`s downstream — semantically equivalent to BA2's
    /// `L2|xxx` fallback for code-switches into unanalyzable
    /// languages.
    fn registry_supporting(langs: &[&str]) -> crate::stanza_registry::StanzaRegistry {
        use crate::types::worker::StanzaLanguageProcessors;
        use std::collections::BTreeMap;
        let mut caps = BTreeMap::new();
        for &iso3 in langs {
            caps.insert(
                iso3.to_string(),
                StanzaLanguageProcessors {
                    alpha2: iso3.chars().take(2).collect(),
                    processors: ["tokenize", "pos", "lemma", "depparse"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
            );
        }
        crate::stanza_registry::StanzaRegistry::from_capabilities(&caps)
    }

    #[test]
    fn partition_groups_by_stanza_support_routes_unsupported_to_fallback() {
        let groups = vec![
            LanguageBatchGroup {
                lang: LanguageCode3::eng(),
                indices: vec![0, 2],
            },
            LanguageBatchGroup {
                lang: LanguageCode3::try_new("nep").unwrap(),
                indices: vec![1],
            },
            LanguageBatchGroup {
                lang: LanguageCode3::try_new("cym").unwrap(),
                indices: vec![3, 4],
            },
        ];
        let registry = registry_supporting(&["eng", "cym"]);
        let (dispatchable, fallback) = partition_groups_by_stanza_support(groups, Some(&registry));
        assert_eq!(
            dispatchable
                .iter()
                .map(|g| g.lang.as_ref())
                .collect::<Vec<_>>(),
            vec!["eng", "cym"]
        );
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].0.as_ref(), "nep");
        assert_eq!(fallback[0].1, vec![1]);
    }

    #[test]
    fn partition_groups_by_stanza_support_with_no_registry_routes_all_to_fallback() {
        // No stanza registry → no support data → safe default is
        // "treat everything as unsupported," producing L2|xxx for
        // all items rather than crashing the worker.
        let groups = vec![LanguageBatchGroup {
            lang: LanguageCode3::eng(),
            indices: vec![0],
        }];
        let (dispatchable, fallback) = partition_groups_by_stanza_support(groups, None);
        assert!(dispatchable.is_empty());
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].0, LanguageCode3::eng());
    }
}
