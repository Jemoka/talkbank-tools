//! Server-side utterance segmentation orchestrator.
//!
//! Owns the full CHAT lifecycle for utseg jobs:
//! parse → collect payloads → infer → apply splits → serialize.
//!
//! Python workers receive only `(words, text) → UtsegResponse` via the infer protocol —
//! pure Stanza constituency parsing with zero CHAT awareness.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::api::{ChatText, EngineVersion, LanguageCode3};
use crate::chat_ops::ChatFile;
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::pool::WorkerPool;
use crate::worker::text_request_v2::{PreparedTextRequestIdsV2, build_utseg_request_v2};
use crate::worker::text_result_v2::parse_utseg_result_v2;
use talkbank_transform::utseg::{
    UtsegBatchItem, UtsegResponse, apply_utseg_results, collect_utseg_payloads,
};

/// Thin adapter matching the legacy `fn(&ChatFile) -> Vec<(usize, Item)>`
/// hook signature. The Wave 5 utseg collector returns the richer
/// [`UtsegPayloadCollection`](talkbank_transform::utseg::UtsegPayloadCollection)
/// struct; this wrapper discards the `not_applicable` outcomes so the
/// existing text-pipeline hooks keep compiling. Surfacing the outcomes
/// through the pipeline is future follow-up work; the data is already
/// typed and available to any caller that calls `collect_utseg_payloads`
/// directly.
fn collect_utseg_batch_items(chat_file: &ChatFile) -> Vec<(usize, UtsegBatchItem)> {
    collect_utseg_payloads(chat_file).batch_items
}
use talkbank_transform::utseg_compute;
use talkbank_transform::validate::ValidityLevel;
use tracing::{info, warn};

use crate::error::ServerError;
use crate::infer_retry::dispatch_execute_v2_with_retry;
use crate::pipeline::PipelineServices;
use crate::pipeline::text_infer::{
    TextBatchHooks, TextPipelineHooks, run_text_batch_pipeline, run_text_pipeline,
};
use crate::text_batch::{
    TextBatchFileInput, TextBatchFileResults, TextBatchOperation, TextBatchWorkflow,
    TextBatchWorkflowRequest, TextPerFileWorkflowRequest,
};

/// Command-specific parameters for the utseg workflow family.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct UtsegWorkflowParams {
    /// Operator opt-in to the legacy Stanza constituency-parser
    /// fallback when no language-specific TalkBank BERT utseg model is
    /// configured. Set by the `--utseg-fallback-stanza` CLI flag.
    pub allow_stanza_fallback: bool,
}

/// Typed workflow operation for utseg.
pub(crate) struct UtsegOperation;

/// Trait-oriented workflow wrapper for utseg.
pub(crate) type UtsegWorkflow = TextBatchWorkflow<UtsegOperation>;

#[async_trait]
impl TextBatchOperation for UtsegOperation {
    type Shared<'a>
        = PipelineServices<'a>
    where
        Self: 'a;

    type Params<'a>
        = UtsegWorkflowParams
    where
        Self: 'a;

    async fn run_single(
        chat_text: ChatText<'_>,
        lang: &LanguageCode3,
        shared: Self::Shared<'_>,
        params: Self::Params<'_>,
    ) -> Result<String, ServerError> {
        run_utseg_impl(
            chat_text.as_ref(),
            lang,
            shared.pool,
            shared.cache,
            shared.engine_version,
            params.allow_stanza_fallback,
        )
        .await
    }

    async fn run_batch(
        files: &[TextBatchFileInput],
        lang: &LanguageCode3,
        shared: Self::Shared<'_>,
        params: Self::Params<'_>,
    ) -> TextBatchFileResults {
        run_utseg_batch_impl(files, lang, shared.pool, params.allow_stanza_fallback).await
    }
}

// ---------------------------------------------------------------------------
// Per-file utseg processing
// ---------------------------------------------------------------------------

/// Process a single CHAT file through the utseg pipeline.
///
/// Returns the serialized CHAT text with utterances split as needed.
pub async fn process_utseg(
    chat_text: &str,
    lang: &LanguageCode3,
    pool: &WorkerPool,
    cache: &crate::cache::UtteranceCache,
    engine_version: &EngineVersion,
    allow_stanza_fallback: bool,
) -> Result<String, ServerError> {
    UtsegWorkflow::new()
        .run_per_file(TextPerFileWorkflowRequest {
            chat_text: ChatText::from(chat_text),
            lang,
            shared: PipelineServices::new(pool, cache, engine_version),
            params: UtsegWorkflowParams {
                allow_stanza_fallback,
            },
        })
        .await
}

/// Infer utterance-boundary assignments for pretokenized word batches.
pub async fn infer_utseg_assignments(
    pool: &WorkerPool,
    lang: &LanguageCode3,
    items: &[UtsegBatchItem],
    allow_stanza_fallback: bool,
) -> Result<Vec<UtsegResponse>, ServerError> {
    let indexed_items: Vec<(usize, UtsegBatchItem)> = items.iter().cloned().enumerate().collect();
    infer_batch(pool, &indexed_items, lang, allow_stanza_fallback).await
}

// ---------------------------------------------------------------------------
// Cross-file batch utseg processing
// ---------------------------------------------------------------------------

/// Process multiple CHAT files, pooling payloads from all files into a single
/// `batch_infer` call for maximum throughput.
///
/// Returns `(filename, Ok(output_text) | Err(error_msg))` for each file.
pub(crate) async fn process_utseg_batch(
    files: &[TextBatchFileInput],
    lang: &LanguageCode3,
    pool: &WorkerPool,
    cache: &crate::cache::UtteranceCache,
    engine_version: &EngineVersion,
    allow_stanza_fallback: bool,
) -> TextBatchFileResults {
    UtsegWorkflow::new()
        .run_batch_files(TextBatchWorkflowRequest {
            files,
            lang,
            shared: PipelineServices::new(pool, cache, engine_version),
            params: UtsegWorkflowParams {
                allow_stanza_fallback,
            },
        })
        .await
}

async fn run_utseg_impl(
    chat_text: &str,
    lang: &LanguageCode3,
    pool: &WorkerPool,
    cache: &crate::cache::UtteranceCache,
    engine_version: &EngineVersion,
    allow_stanza_fallback: bool,
) -> Result<String, ServerError> {
    run_text_pipeline(
        chat_text,
        lang,
        PipelineServices::new(pool, cache, engine_version),
        TextPipelineHooks {
            command: "utseg",
            validity: ValidityLevel::StructurallyComplete,
            collect: collect_utseg_batch_items,
            integrate: integrate_assignments,
            apply: apply_utseg_results,
        },
        // The generic pipeline's `infer` signature doesn't carry
        // command-specific state, so capture the operator opt-in here
        // and bind it onto each `infer_batch` invocation.
        async move |pool, items, lang| infer_batch(pool, items, lang, allow_stanza_fallback).await,
    )
    .await
}

async fn run_utseg_batch_impl(
    files: &[TextBatchFileInput],
    lang: &LanguageCode3,
    pool: &WorkerPool,
    allow_stanza_fallback: bool,
) -> TextBatchFileResults {
    run_text_batch_pipeline(
        files,
        lang,
        pool,
        TextBatchHooks {
            command: "utseg",
            validity: ValidityLevel::StructurallyComplete,
            collect: collect_utseg_batch_items,
            apply: apply_utseg_file,
        },
        async move |pool, items, lang| infer_batch(pool, items, lang, allow_stanza_fallback).await,
    )
    .await
}

/// Apply utseg responses for one file, skipping items with a
/// length-mismatched assignment vector.
fn apply_utseg_file(
    chat_file: &mut ChatFile,
    items: &[(usize, UtsegBatchItem)],
    responses: &[UtsegResponse],
) {
    let mut assignment_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for ((utt_ordinal, item), resp) in items.iter().zip(responses.iter()) {
        if resp.assignments.len() == item.words.len() {
            assignment_map.insert(*utt_ordinal, resp.assignments.clone());
        } else {
            warn!(
                utterance = utt_ordinal,
                expected = item.words.len(),
                got = resp.assignments.len(),
                "utseg assignment length mismatch, keeping original"
            );
        }
    }
    if !assignment_map.is_empty() {
        apply_utseg_results(chat_file, &assignment_map);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Send batch items to a worker for constituency inference via batched
/// `execute_v2`.
///
/// `allow_stanza_fallback` propagates the operator opt-in
/// (`--utseg-fallback-stanza`) to the worker so it can engage the
/// legacy Stanza constituency-parser fallback when no
/// language-specific BERT utseg model is configured.
async fn infer_batch(
    pool: &WorkerPool,
    items: &[(usize, UtsegBatchItem)],
    lang: &LanguageCode3,
    allow_stanza_fallback: bool,
) -> Result<Vec<UtsegResponse>, ServerError> {
    let payload_items: Vec<_> = items.iter().map(|(_, item)| item.clone()).collect();
    let artifacts = PreparedArtifactRuntimeV2::new("utseg_v2").map_err(|error| {
        ServerError::Validation(format!(
            "failed to create utseg V2 artifact runtime: {error}"
        ))
    })?;
    let request_ids = PreparedTextRequestIdsV2::for_task("utseg");
    let request = build_utseg_request_v2(
        artifacts.store(),
        &request_ids,
        lang,
        &payload_items,
        allow_stanza_fallback,
    )
    .map_err(|error| {
        ServerError::Validation(format!("failed to build utseg V2 worker request: {error}"))
    })?;

    info!(
        num_items = items.len(),
        lang = %lang,
        "Dispatching utseg execute_v2 batch"
    );

    let response = dispatch_execute_v2_with_retry(pool, lang, &request).await?;
    let result = parse_utseg_result_v2(&response)
        .map_err(|error| ServerError::Validation(format!("invalid utseg V2 result: {error}")))?;
    if result.items.len() != items.len() {
        return Err(ServerError::Validation(format!(
            "utseg V2 returned {} items for {} requests",
            result.items.len(),
            items.len()
        )));
    }

    let mut utseg_responses = Vec::with_capacity(result.items.len());
    for (i, item_result) in result.items.iter().enumerate() {
        if let Some(error) = &item_result.error {
            warn!(item = i, error = %error, "Infer error for item (using empty response)");
            utseg_responses.push(UtsegResponse {
                assignments: Vec::new(),
            });
            continue;
        }

        if let Some(assignments) = &item_result.assignments {
            utseg_responses.push(UtsegResponse {
                assignments: assignments.clone(),
            });
            continue;
        }

        if let Some(trees) = &item_result.trees {
            let num_words = items[i].1.words.len();
            let assignments = utseg_compute::compute_assignments(trees, num_words);
            utseg_responses.push(UtsegResponse { assignments });
            continue;
        }

        warn!(
            item = i,
            "Utseg V2 returned no assignments, no trees, and no error (using empty response)"
        );
        utseg_responses.push(UtsegResponse {
            assignments: Vec::new(),
        });
    }

    Ok(utseg_responses)
}

fn integrate_assignments(
    assignment_map: &mut HashMap<usize, Vec<usize>>,
    misses: &[(usize, UtsegBatchItem)],
    responses: &[UtsegResponse],
) {
    for ((utt_ordinal, item), resp) in misses.iter().zip(responses.iter()) {
        if resp.assignments.len() == item.words.len() {
            assignment_map.insert(*utt_ordinal, resp.assignments.clone());
        } else {
            warn!(
                utterance = utt_ordinal,
                expected = item.words.len(),
                got = resp.assignments.len(),
                "utseg assignment length mismatch, keeping original"
            );
        }
    }
}
