//! Server-side coreference resolution orchestrator.
//!
//! Owns the full CHAT lifecycle for coref jobs:
//! parse → collect sentences → check language → infer → inject %xcoref → serialize.
//!
//! Key differences from morphosyntax/utseg/translate:
//! - **Document-level**: Each worker item is one complete document, not one utterance.
//! - **No caching**: Results depend on full document context — per-utterance caching is meaningless.
//! - **English-only**: Non-English files are passed through unchanged.
//! - **Sparse injection**: Only utterances with actual coref chains get `%xcoref`.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::api::{ChatText, LanguageCode3};
use crate::chat_ops::LanguageCode;
use crate::chat_ops::morphosyntax_ops::declared_languages;
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::pool::WorkerPool;
use crate::worker::text_request_v2::{PreparedTextRequestIdsV2, build_coref_request_v2};
use crate::worker::text_result_v2::parse_coref_result_v2;
use talkbank_transform::coref::{
    ChainRef, CorefBatchItem, CorefRawAnnotation, CorefRawResponse, CorefResponse,
    apply_coref_results, collect_coref_payloads, raw_to_bracket_response,
};
use talkbank_transform::parse::{is_dummy, parse_lenient};
use talkbank_transform::serialize::to_chat_string;
use talkbank_transform::validate::{ValidityLevel, validate_output, validate_to_level};
use tracing::{info, warn};

use crate::error::ServerError;
use crate::infer_retry::dispatch_execute_v2_with_retry;
use crate::text_batch::{
    TextBatchFileInput, TextBatchFileResult, TextBatchFileResults, TextBatchOperation,
    TextBatchWorkflow, TextBatchWorkflowRequest, TextPerFileWorkflowRequest,
};

/// Check whether a parsed CHAT file declares English as one of its languages.
///
/// Uses the per-file `@Languages` header (via `declared_languages()`); files
/// with no `@Languages` header fall back to `eng` (BA2 parity — coref is an
/// English-only command, so the fallback is fixed). The `--lang` flag was
/// removed for BA2 parity (`~/batchalign2-master/batchalign/cli/cli.py:276`
/// has no `--lang` for coref); see the 2026-05-03 incident for why a
/// job-level lang sentinel is unsafe.
fn file_has_english(chat_file: &crate::chat_ops::ChatFile) -> bool {
    let fallback = LanguageCode::new(LanguageCode3::eng().as_ref());
    let langs = declared_languages(chat_file, &fallback);
    langs.iter().any(|l| l.as_str() == "eng")
}

/// Typed workflow operation for coref.
pub(crate) struct CorefOperation;

/// Trait-oriented workflow wrapper for coref.
pub(crate) type CorefWorkflow = TextBatchWorkflow<CorefOperation>;

#[async_trait]
impl TextBatchOperation for CorefOperation {
    type Shared<'a>
        = &'a WorkerPool
    where
        Self: 'a;

    type Params<'a>
        = ()
    where
        Self: 'a;

    async fn run_single(
        chat_text: ChatText<'_>,
        lang: &LanguageCode3,
        pool: Self::Shared<'_>,
        _params: Self::Params<'_>,
    ) -> Result<String, ServerError> {
        run_coref_impl(chat_text.as_ref(), lang, pool).await
    }

    async fn run_batch(
        files: &[TextBatchFileInput],
        lang: &LanguageCode3,
        pool: Self::Shared<'_>,
        _params: Self::Params<'_>,
    ) -> TextBatchFileResults {
        run_coref_batch_impl(files, lang, pool).await
    }
}

// ---------------------------------------------------------------------------
// Per-file coref processing
// ---------------------------------------------------------------------------

/// Process a single CHAT file through the coreference resolution pipeline.
///
/// Returns the serialized CHAT text with `%xcoref` tiers injected.
/// Non-English files are returned as-is (checked via per-file `@Languages`).
pub async fn process_coref(
    chat_text: &str,
    lang: &LanguageCode3,
    pool: &WorkerPool,
) -> Result<String, ServerError> {
    CorefWorkflow::new()
        .run_per_file(TextPerFileWorkflowRequest {
            chat_text: ChatText::from(chat_text),
            lang,
            shared: pool,
            params: (),
        })
        .await
}

async fn run_coref_impl(
    chat_text: &str,
    lang: &LanguageCode3,
    pool: &WorkerPool,
) -> Result<String, ServerError> {
    let parser = crate::chat_parser();
    // 1. Parse
    let (mut chat_file, parse_errors) = parse_lenient(&parser, chat_text);
    if !parse_errors.is_empty() {
        warn!(
            num_errors = parse_errors.len(),
            "Parse errors in coref input (continuing with recovery)"
        );
    }

    // 1b. Skip dummy files
    if is_dummy(&chat_file) {
        return Ok(to_chat_string(&chat_file));
    }

    // 1c. Pre-validation gate (L1: StructurallyComplete)
    if let Err(errors) = validate_to_level(
        &chat_file,
        &parse_errors,
        ValidityLevel::StructurallyComplete,
    ) {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(ServerError::Validation(format!(
            "coref pre-validation failed: {}",
            msgs.join("; ")
        )));
    }

    // 2. English-only gate (per-file @Languages, not job-level lang)
    if !file_has_english(&chat_file) {
        return Ok(to_chat_string(&chat_file));
    }

    // 3. Collect payloads
    let collected = collect_coref_payloads(&chat_file);
    let coref_item = collected.batch_item;
    let line_indices = collected.line_indices;
    // Wave 5: collected.not_applicable carries typed NotApplicable
    // outcomes for empty utterances. Not surfaced through the reporting
    // tier here; available to any caller that wants them.

    if coref_item.sentences.is_empty() {
        return Ok(to_chat_string(&chat_file));
    }

    // 4. Infer via worker
    let mut coref_responses = infer_batch(
        pool,
        std::slice::from_ref(&coref_item),
        &LanguageCode3::eng(),
    )
    .await?;
    let coref_response = coref_responses.pop().unwrap_or(CorefResponse {
        annotations: Vec::new(),
    });

    // 5. Map sentence_idx → line_idx and build results map
    let mut results: HashMap<usize, String> = HashMap::new();
    for ann in &coref_response.annotations {
        if ann.sentence_idx < line_indices.len() {
            let line_idx = line_indices[ann.sentence_idx];
            results.insert(line_idx, ann.annotation.clone());
        } else {
            warn!(
                sentence_idx = ann.sentence_idx,
                num_sentences = line_indices.len(),
                "Coref annotation sentence_idx out of range"
            );
        }
    }

    // 6. Apply annotations
    apply_coref_results(&mut chat_file, &results);

    // 7. Post-validation check (warn only — always serialize output for debugging).
    if let Err(errors) = validate_output(&chat_file, "coref") {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        warn!(errors = ?msgs, "coref post-validation warnings (non-fatal)");
    }

    // 8. Inject provenance + serialize. Coref is English-only, so the
    // provenance lang is hardcoded `eng` regardless of any value the
    // gateway/dispatch handed in. The `lang: &LanguageCode3` parameter on
    // this function is retained for shared-trait symmetry with the other
    // text commands but is otherwise unused — see the 2026-05-03 incident
    // for why a job-level lang must not flow into provenance.
    let _ = lang; // intentionally ignored
    let provenance = crate::provenance::coref_provenance(LanguageCode3::eng().as_ref(), "stanza");
    crate::provenance::inject_provenance(&mut chat_file, &provenance);
    Ok(to_chat_string(&chat_file))
}

// ---------------------------------------------------------------------------
// Cross-file batch coref processing
// ---------------------------------------------------------------------------

/// Process multiple CHAT files, sending one `CorefBatchItem` per eligible file
/// in a single batched `execute_v2` call.
///
/// Returns `(filename, Ok(output_text) | Err(error_msg))` for each file.
pub(crate) async fn process_coref_batch(
    files: &[TextBatchFileInput],
    lang: &LanguageCode3,
    pool: &WorkerPool,
) -> TextBatchFileResults {
    CorefWorkflow::new()
        .run_batch_files(TextBatchWorkflowRequest {
            files,
            lang,
            shared: pool,
            params: (),
        })
        .await
}

async fn run_coref_batch_impl(
    files: &[TextBatchFileInput],
    _lang: &LanguageCode3,
    pool: &WorkerPool,
) -> TextBatchFileResults {
    // `_lang` is intentionally unused. Coref is English-only (BA2 parity),
    // so per-file English-ness is read from each file's `@Languages:` header
    // (`file_has_english`) and the inference language is hardcoded to
    // `LanguageCode3::eng()`. The parameter remains in the signature for
    // shared-trait symmetry with utseg/translate. See the 2026-05-03
    // morphotag incident for why a job-level lang must not flow through.
    let parser = crate::chat_parser();
    let mut results: TextBatchFileResults = Vec::with_capacity(files.len());

    // 1. Parse all files
    let mut parsed_files: Vec<crate::chat_ops::ChatFile> = Vec::with_capacity(files.len());
    let mut parse_error_lists: Vec<Vec<crate::chat_ops::ParseError>> =
        Vec::with_capacity(files.len());
    for file in files {
        let filename = file.filename.as_ref();
        let (chat_file, parse_errors) = parse_lenient(&parser, file.chat_text.as_ref());
        if !parse_errors.is_empty() {
            warn!(
                filename = %filename,
                num_errors = parse_errors.len(),
                "Parse errors (continuing with recovery)"
            );
        }
        parse_error_lists.push(parse_errors);
        parsed_files.push(chat_file);
    }

    // 2. Collect payloads per file (per-file English gate)
    struct FileCorefInfo {
        line_indices: Vec<usize>,
        batch_idx: usize, // index into the execute_v2 batch array
    }

    let mut eligible_files: Vec<(usize, FileCorefInfo)> = Vec::new();
    let mut batch_items: Vec<CorefBatchItem> = Vec::new();
    let mut validation_errors: Vec<Option<String>> = vec![None; files.len()];

    for (file_idx, parsed_file) in parsed_files.iter().enumerate() {
        // Skip dummy files — they pass through unchanged
        if is_dummy(parsed_file) {
            continue;
        }

        // Pre-validation gate (L1: StructurallyComplete)
        if let Err(errors) = validate_to_level(
            parsed_file,
            &parse_error_lists[file_idx],
            ValidityLevel::StructurallyComplete,
        ) {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            validation_errors[file_idx] =
                Some(format!("coref pre-validation failed: {}", msgs.join("; ")));
            continue;
        }

        // Per-file English-only gate — non-English files pass through unchanged
        if !file_has_english(parsed_file) {
            continue;
        }

        let collected = collect_coref_payloads(parsed_file);
        let coref_item = collected.batch_item;
        let line_indices = collected.line_indices;

        if coref_item.sentences.is_empty() {
            continue;
        }

        let batch_idx = batch_items.len();
        batch_items.push(coref_item);
        eligible_files.push((
            file_idx,
            FileCorefInfo {
                line_indices,
                batch_idx,
            },
        ));
    }

    // 3. Single batched execute_v2 call across all files
    let all_responses = if batch_items.is_empty() {
        Vec::new()
    } else {
        info!(
            num_items = batch_items.len(),
            "Dispatching coref execute_v2 batch"
        );

        match infer_batch(pool, &batch_items, &LanguageCode3::eng()).await {
            Ok(responses) => responses,
            Err(e) => {
                warn!(error = %e, "Batch coref execute_v2 failed for all files");
                // Return all files serialized without coref
                for (file_idx, file) in files.iter().enumerate() {
                    results.push(TextBatchFileResult::ok(
                        file.filename.clone(),
                        to_chat_string(&parsed_files[file_idx]),
                    ));
                }
                return results;
            }
        }
    };

    // 4. Distribute responses back to files and apply
    for &(file_idx, ref info) in &eligible_files {
        if info.batch_idx < all_responses.len() {
            let coref_resp = &all_responses[info.batch_idx];

            let mut annotation_map: HashMap<usize, String> = HashMap::new();
            for ann in &coref_resp.annotations {
                if ann.sentence_idx < info.line_indices.len() {
                    let line_idx = info.line_indices[ann.sentence_idx];
                    annotation_map.insert(line_idx, ann.annotation.clone());
                }
            }

            if !annotation_map.is_empty() {
                apply_coref_results(&mut parsed_files[file_idx], &annotation_map);
            }
        }
    }

    // 5. Serialize all files
    for (file_idx, file) in files.iter().enumerate() {
        let filename = file.filename.as_ref();
        // Skip files that failed pre-validation
        if let Some(ref err) = validation_errors[file_idx] {
            results.push(TextBatchFileResult::err(file.filename.clone(), err.clone()));
            continue;
        }

        // Post-validation check (warn only — always serialize output for debugging).
        if let Err(errors) = validate_output(&parsed_files[file_idx], "coref") {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            warn!(filename = %filename, errors = ?msgs, "coref post-validation warnings (non-fatal)");
        }

        results.push(TextBatchFileResult::ok(
            file.filename.clone(),
            to_chat_string(&parsed_files[file_idx]),
        ));
    }

    results
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Send one or more documents to a worker for coref inference via batched
/// `execute_v2`.
async fn infer_batch(
    pool: &WorkerPool,
    items: &[CorefBatchItem],
    lang: &LanguageCode3,
) -> Result<Vec<CorefResponse>, ServerError> {
    let artifacts = PreparedArtifactRuntimeV2::new("coref_v2").map_err(|error| {
        ServerError::Validation(format!(
            "failed to create coref V2 artifact runtime: {error}"
        ))
    })?;
    let request_ids = PreparedTextRequestIdsV2::for_task("coref");
    let request =
        build_coref_request_v2(artifacts.store(), &request_ids, lang, items).map_err(|error| {
            ServerError::Validation(format!("failed to build coref V2 worker request: {error}"))
        })?;

    let response = dispatch_execute_v2_with_retry(pool, lang, &request).await?;
    let result = parse_coref_result_v2(&response)
        .map_err(|error| ServerError::Validation(format!("invalid coref V2 result: {error}")))?;
    if result.items.len() != items.len() {
        return Err(ServerError::Validation(format!(
            "coref V2 returned {} items for {} requests",
            result.items.len(),
            items.len()
        )));
    }

    let mut responses = Vec::with_capacity(result.items.len());
    for (index, item_result) in result.items.iter().enumerate() {
        if let Some(error) = &item_result.error {
            warn!(item = index, error = %error, "Coref infer error (using empty response)");
            responses.push(CorefResponse {
                annotations: Vec::new(),
            });
            continue;
        }

        responses.push(coref_response_from_v2_item(item_result));
    }

    Ok(responses)
}

/// Convert one typed V2 coref item result into the established Rust response.
fn coref_response_from_v2_item(
    item_result: &crate::types::worker_v2::CorefItemResultV2,
) -> CorefResponse {
    let raw = CorefRawResponse {
        annotations: item_result
            .annotations
            .as_ref()
            .map(|annotations| {
                annotations
                    .iter()
                    .map(|annotation| CorefRawAnnotation {
                        sentence_idx: annotation.sentence_idx,
                        words: annotation
                            .words
                            .iter()
                            .map(|word_refs| {
                                word_refs
                                    .iter()
                                    .map(|chain_ref| ChainRef {
                                        chain_id: chain_ref.chain_id,
                                        is_start: chain_ref.is_start,
                                        is_end: chain_ref.is_end,
                                    })
                                    .collect()
                            })
                            .collect(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    raw_to_bracket_response(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_transform::parse::TreeSitterParser;

    #[test]
    fn test_file_has_english_with_eng_languages() {
        let parser = TreeSitterParser::new().unwrap();
        let chat = include_str!("../../../../resources/fixtures/eng_hello_world.cha");
        let (chat_file, _) = parse_lenient(&parser, chat);
        assert!(file_has_english(&chat_file));
    }

    #[test]
    fn test_file_has_english_with_spa_languages() {
        let parser = TreeSitterParser::new().unwrap();
        let chat = include_str!("../../../../resources/fixtures/spa_chi_hola_mundo.cha");
        let (chat_file, _) = parse_lenient(&parser, chat);
        assert!(!file_has_english(&chat_file));
    }

    #[test]
    fn test_file_has_english_no_languages_header_uses_eng_fallback() {
        // BA2 parity: coref's English-only fallback for missing @Languages is
        // hardcoded `eng` (--lang was removed from the CLI). A file without
        // an @Languages header is treated as English.
        let parser = TreeSitterParser::new().unwrap();
        let chat = include_str!("../../../../resources/fixtures/eng_hello_world_no_languages.cha");
        let (chat_file, _) = parse_lenient(&parser, chat);
        assert!(file_has_english(&chat_file));
    }

    #[test]
    fn test_file_has_english_multilingual_with_eng() {
        let parser = TreeSitterParser::new().unwrap();
        // File declares both eng and spa — should be considered English
        let chat = include_str!("../../../../resources/fixtures/eng_spa_bilingual_hello_world.cha");
        let (chat_file, _) = parse_lenient(&parser, chat);
        assert!(file_has_english(&chat_file));
    }
}
