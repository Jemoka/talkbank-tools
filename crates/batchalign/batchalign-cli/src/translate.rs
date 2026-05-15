//! Server-side translation orchestrator.
//!
//! Owns the full CHAT lifecycle for translate jobs:
//! parse → collect payloads → infer → inject %xtra → serialize.
//!
//! Python workers receive only `(text) → TranslateResponse` via the infer protocol —
//! pure Google Translate / Seamless inference with zero CHAT awareness.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::api::{ChatText, EngineVersion, LanguageCode3};
use crate::chat_ops::{ChatFile, LanguageCode};
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::pool::WorkerPool;
use crate::worker::text_request_v2::{PreparedTextRequestIdsV2, build_translate_request_v2};
use crate::worker::text_result_v2::parse_translate_result_v2;
use talkbank_transform::translate::{
    TranslateBatchItem, TranslateResponse, apply_translate_results, chat_punct_chars,
    collect_translate_payloads, postprocess_translation, preprocess_for_translate,
};
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

/// Command-specific parameters for the translate workflow family.
///
/// Retained as a zero-field struct so the `TextBatchOperation` shape stays
/// symmetric with morphotag/utseg; may grow again when translate acquires
/// real command-specific options.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TranslateWorkflowParams;

/// Typed workflow operation for translate.
pub(crate) struct TranslateOperation;

/// Trait-oriented workflow wrapper for translate.
pub(crate) type TranslateWorkflow = TextBatchWorkflow<TranslateOperation>;

#[async_trait]
impl TextBatchOperation for TranslateOperation {
    type Shared<'a>
        = PipelineServices<'a>
    where
        Self: 'a;

    type Params<'a>
        = TranslateWorkflowParams
    where
        Self: 'a;

    async fn run_single(
        chat_text: ChatText<'_>,
        lang: &LanguageCode3,
        shared: Self::Shared<'_>,
        _params: Self::Params<'_>,
    ) -> Result<String, ServerError> {
        run_translate_impl(
            chat_text.as_ref(),
            lang,
            shared.pool,
            shared.cache,
            shared.engine_version,
        )
        .await
    }

    async fn run_batch(
        files: &[TextBatchFileInput],
        lang: &LanguageCode3,
        shared: Self::Shared<'_>,
        _params: Self::Params<'_>,
    ) -> TextBatchFileResults {
        run_translate_batch_impl(files, lang, shared.pool).await
    }
}

// ---------------------------------------------------------------------------
// Per-file translate processing
// ---------------------------------------------------------------------------

/// Process a single CHAT file through the translation pipeline.
///
/// Returns the serialized CHAT text with `%xtra` tiers injected.
pub async fn process_translate(
    chat_text: &str,
    lang: &LanguageCode3,
    pool: &WorkerPool,
    cache: &crate::cache::UtteranceCache,
    engine_version: &EngineVersion,
) -> Result<String, ServerError> {
    TranslateWorkflow::new()
        .run_per_file(TextPerFileWorkflowRequest {
            chat_text: ChatText::from(chat_text),
            lang,
            shared: PipelineServices::new(pool, cache, engine_version),
            params: TranslateWorkflowParams,
        })
        .await
}

// ---------------------------------------------------------------------------
// Cross-file batch translate processing
// ---------------------------------------------------------------------------

/// Process multiple CHAT files, pooling all payloads into a single
/// `batch_infer` call for maximum throughput.
///
/// Returns `(filename, Ok(output_text) | Err(error_msg))` for each file.
pub(crate) async fn process_translate_batch(
    files: &[TextBatchFileInput],
    lang: &LanguageCode3,
    pool: &WorkerPool,
    cache: &crate::cache::UtteranceCache,
    engine_version: &EngineVersion,
) -> TextBatchFileResults {
    TranslateWorkflow::new()
        .run_batch_files(TextBatchWorkflowRequest {
            files,
            lang,
            shared: PipelineServices::new(pool, cache, engine_version),
            params: TranslateWorkflowParams,
        })
        .await
}

async fn run_translate_impl(
    chat_text: &str,
    lang: &LanguageCode3,
    pool: &WorkerPool,
    cache: &crate::cache::UtteranceCache,
    engine_version: &EngineVersion,
) -> Result<String, ServerError> {
    run_text_pipeline(
        chat_text,
        lang,
        PipelineServices::new(pool, cache, engine_version),
        TextPipelineHooks {
            command: "translate",
            validity: ValidityLevel::StructurallyComplete,
            collect: collect_translate_payloads,
            integrate: integrate_translations,
            apply: apply_translate_results,
        },
        infer_batch,
    )
    .await
}

async fn run_translate_batch_impl(
    files: &[TextBatchFileInput],
    lang: &LanguageCode3,
    pool: &WorkerPool,
) -> TextBatchFileResults {
    run_text_batch_pipeline(
        files,
        lang,
        pool,
        TextBatchHooks {
            command: "translate",
            validity: ValidityLevel::StructurallyComplete,
            collect: collect_translate_payloads,
            apply: apply_translate_file,
        },
        infer_batch,
    )
    .await
}

/// Apply translate responses for one file, skipping items with an
/// empty translation.
fn apply_translate_file(
    chat_file: &mut ChatFile,
    items: &[(usize, TranslateBatchItem)],
    responses: &[TranslateResponse],
) {
    let mut translation_map: HashMap<usize, String> = HashMap::new();
    for ((line_idx, _item), resp) in items.iter().zip(responses.iter()) {
        if !resp.translation.is_empty() {
            translation_map.insert(*line_idx, resp.translation.clone());
        }
    }
    if !translation_map.is_empty() {
        apply_translate_results(chat_file, &translation_map);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Send batch items to a worker for translation inference via batched
/// `execute_v2`.
///
/// Applies pre-processing (Chinese space removal) before sending to Python
/// and post-processing (punct spacing, quote normalization) on the raw response.
async fn infer_batch(
    pool: &WorkerPool,
    items: &[(usize, TranslateBatchItem)],
    lang: &LanguageCode3,
) -> Result<Vec<TranslateResponse>, ServerError> {
    let src_lang_code = LanguageCode::new(lang.as_ref());

    // Pre-process text before sending to Python
    let preprocessed_items: Vec<TranslateBatchItem> = items
        .iter()
        .map(|(_, item)| TranslateBatchItem {
            text: preprocess_for_translate(&item.text, &src_lang_code),
        })
        .collect();
    let artifacts = PreparedArtifactRuntimeV2::new("translate_v2").map_err(|error| {
        ServerError::Validation(format!(
            "failed to create translate V2 artifact runtime: {error}"
        ))
    })?;
    let request_ids = PreparedTextRequestIdsV2::for_task("translate");
    let target_lang = LanguageCode3::eng();
    let request = build_translate_request_v2(
        artifacts.store(),
        &request_ids,
        lang,
        &target_lang,
        &preprocessed_items,
    )
    .map_err(|error| {
        ServerError::Validation(format!(
            "failed to build translate V2 worker request: {error}"
        ))
    })?;

    info!(
        num_items = items.len(),
        lang = %lang,
        "Dispatching translate execute_v2 batch"
    );

    let response = dispatch_execute_v2_with_retry(pool, lang, &request).await?;
    let result = parse_translate_result_v2(&response).map_err(|error| {
        ServerError::Validation(format!("invalid translate V2 result: {error}"))
    })?;
    if result.items.len() != items.len() {
        return Err(ServerError::Validation(format!(
            "translate V2 returned {} items for {} requests",
            result.items.len(),
            items.len()
        )));
    }

    // Get punctuation chars for post-processing
    let punct_strings = chat_punct_chars();
    let punct_refs: Vec<&str> = punct_strings.iter().map(|s| s.as_str()).collect();

    let mut translate_responses = Vec::with_capacity(result.items.len());
    for (i, item_result) in result.items.iter().enumerate() {
        if let Some(error) = &item_result.error {
            warn!(item = i, error = %error, "Infer error for item (using empty response)");
            translate_responses.push(TranslateResponse {
                translation: String::new(),
            });
            continue;
        }

        if let Some(raw_translation) = &item_result.raw_translation {
            let processed = postprocess_translation(raw_translation, &punct_refs);
            translate_responses.push(TranslateResponse {
                translation: processed,
            });
            continue;
        }

        warn!(
            item = i,
            "Translate V2 returned no raw_translation and no error (using empty response)"
        );
        translate_responses.push(TranslateResponse {
            translation: String::new(),
        });
    }

    Ok(translate_responses)
}

fn integrate_translations(
    translation_map: &mut HashMap<usize, String>,
    misses: &[(usize, TranslateBatchItem)],
    responses: &[TranslateResponse],
) {
    for ((line_idx, _item), resp) in misses.iter().zip(responses.iter()) {
        if !resp.translation.is_empty() {
            translation_map.insert(*line_idx, resp.translation.clone());
        }
    }
}
