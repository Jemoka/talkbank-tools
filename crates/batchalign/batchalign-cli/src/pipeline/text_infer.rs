//! Shared text-infer pipeline skeleton: collect payloads, run worker
//! inference, apply results back to the CHAT AST. Supports both
//! single-file and cross-file-batch flows.

use std::collections::HashMap;

use crate::api::LanguageCode3;
use crate::chat_ops::ChatFile;
use crate::text_batch::{TextBatchFileInput, TextBatchFileResult, TextBatchFileResults};
use crate::worker::pool::WorkerPool;
use talkbank_transform::parse::{is_dummy, parse_lenient};
use talkbank_transform::serialize::to_chat_string;
use talkbank_transform::validate::{ValidityLevel, validate_output, validate_to_level};
use tracing::warn;

use crate::error::ServerError;
use crate::pipeline::PipelineServices;

type IntegrateFn<Item, State, Response> =
    fn(&mut HashMap<usize, State>, &[(usize, Item)], &[Response]);

/// Hooks for a text-only single-file pipeline.
///
/// `collect` extracts payloads from the parsed CHAT; `integrate` merges
/// responses into the state map; `apply` writes the state back into
/// the CHAT AST. The inference function itself is a separate argument
/// to [`run_text_pipeline`] so callers can pass any `async fn` directly.
pub(crate) struct TextPipelineHooks<Item, State, Response> {
    /// User-visible command name for validation and error strings.
    pub command: &'static str,
    /// Pre-validation gate required by the command.
    pub validity: ValidityLevel,
    /// Extract worker payloads from the parsed chat file.
    pub collect: fn(&ChatFile) -> Vec<(usize, Item)>,
    /// Merge inferred responses into the final application map.
    pub integrate: IntegrateFn<Item, State, Response>,
    /// Apply all results to the parsed chat file.
    pub apply: fn(&mut ChatFile, &HashMap<usize, State>),
}

/// Run the text-only pipeline for a single CHAT file.
///
/// `infer` runs worker inference for all collected payloads. It is an
/// `async` callable (stable `AsyncFnOnce` trait, Rust 2024), so native
/// `async fn` inference routines can be passed without a boxed-future
/// adapter at the call site.
pub(crate) async fn run_text_pipeline<Item, State, Response, Infer>(
    chat_text: &str,
    lang: &LanguageCode3,
    services: PipelineServices<'_>,
    hooks: TextPipelineHooks<Item, State, Response>,
    infer: Infer,
) -> Result<String, ServerError>
where
    Infer: AsyncFnOnce(
        &WorkerPool,
        &[(usize, Item)],
        &LanguageCode3,
    ) -> Result<Vec<Response>, ServerError>,
{
    let parser = crate::chat_parser();
    let (mut chat_file, parse_errors) = parse_lenient(&parser, chat_text);
    if !parse_errors.is_empty() {
        warn!(
            command = hooks.command,
            num_errors = parse_errors.len(),
            "Parse errors in input (continuing with recovery)"
        );
    }

    if is_dummy(&chat_file) {
        return Ok(to_chat_string(&chat_file));
    }

    if let Err(errors) = validate_to_level(&chat_file, &parse_errors, hooks.validity) {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(ServerError::Validation(format!(
            "{} pre-validation failed: {}",
            hooks.command,
            msgs.join("; ")
        )));
    }

    let batch_items = (hooks.collect)(&chat_file);
    if batch_items.is_empty() {
        return Ok(to_chat_string(&chat_file));
    }

    let responses = infer(services.pool, &batch_items, lang).await?;
    let mut state_map: HashMap<usize, State> = HashMap::new();
    (hooks.integrate)(&mut state_map, &batch_items, &responses);

    (hooks.apply)(&mut chat_file, &state_map);

    if let Err(errors) = validate_output(&chat_file, hooks.command) {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        warn!(command = hooks.command, errors = ?msgs, "post-validation warnings (non-fatal)");
    }

    // Inject processing provenance comment.
    let ev = services.engine_version.as_ref();
    let lang_str = lang.as_ref();
    let provenance = match hooks.command {
        "utseg" => Some(crate::provenance::utseg_provenance(lang_str, ev)),
        "translate" => Some(crate::provenance::translate_provenance(lang_str, ev)),
        _ => None,
    };
    if let Some(comment) = provenance {
        crate::provenance::inject_provenance(&mut chat_file, &comment);
    }

    Ok(to_chat_string(&chat_file))
}

/// Hooks for the cross-file text-batch pipeline (pool all files'
/// payloads into one `batch_infer` call, then redistribute responses).
///
/// Shared between `utseg` and `translate`. Morphotag does not use this
/// because it has additional structure (language-group dispatch, L2
/// secondary dispatch, alignment validation) that doesn't fit the
/// generic shape.
/// Extract worker payloads from a parsed chat file. Each payload is tagged
/// with its utterance index so the injector can match responses back.
pub(crate) type TextBatchCollect<Item> = fn(&ChatFile) -> Vec<(usize, Item)>;

/// Apply one file's collected items + their worker responses to the chat
/// file's AST. Called once per file after global inference completes.
pub(crate) type TextBatchApply<Item, Response> = fn(&mut ChatFile, &[(usize, Item)], &[Response]);

pub(crate) struct TextBatchHooks<Item, Response> {
    /// User-visible command name for validation and log messages.
    pub command: &'static str,
    /// Pre-validation gate required by the command.
    pub validity: ValidityLevel,
    /// Extract worker payloads from the parsed chat file.
    pub collect: TextBatchCollect<Item>,
    /// Apply one file's items + responses directly to that file's AST.
    /// Called once per file after global inference completes.
    pub apply: TextBatchApply<Item, Response>,
}

/// Run the cross-file text-batch pipeline for `files`.
///
/// The pipeline:
/// 1. Parses all files once.
/// 2. For each non-dummy file, validates and collects payloads,
///    recording a `{item_count, global_start}` slice into the pooled
///    payload vector.
/// 3. Calls `infer` once over every file's payloads together.
/// 4. Slices the pooled responses back to each file and invokes
///    `hooks.apply` to inject results into the AST.
/// 5. Runs post-validation (warn-only) and serializes each file.
///
/// On worker failure every file whose items went into the batch is
/// reported as an error; files with no payloads (empty/dummy) are
/// serialized unchanged.
pub(crate) async fn run_text_batch_pipeline<Item, Response, Infer>(
    files: &[TextBatchFileInput],
    lang: &LanguageCode3,
    pool: &WorkerPool,
    hooks: TextBatchHooks<Item, Response>,
    infer: Infer,
) -> TextBatchFileResults
where
    Infer: AsyncFnOnce(
        &WorkerPool,
        &[(usize, Item)],
        &LanguageCode3,
    ) -> Result<Vec<Response>, ServerError>,
{
    let parser = crate::chat_parser();
    let mut results: TextBatchFileResults = Vec::with_capacity(files.len());

    // 1. Parse every input.
    let mut parsed_files: Vec<ChatFile> = Vec::with_capacity(files.len());
    let mut parse_error_lists: Vec<Vec<crate::chat_ops::ParseError>> =
        Vec::with_capacity(files.len());
    for file in files {
        let (chat_file, parse_errors) = parse_lenient(&parser, file.chat_text.as_ref());
        if !parse_errors.is_empty() {
            warn!(
                filename = %file.filename.as_ref(),
                num_errors = parse_errors.len(),
                "Parse errors (continuing with recovery)"
            );
        }
        parse_error_lists.push(parse_errors);
        parsed_files.push(chat_file);
    }

    // 2. Pool payloads across files, remembering each file's slice.
    struct PerFileBatch {
        item_count: usize,
        global_start: usize,
    }

    let mut all_items: Vec<(usize, Item)> = Vec::new();
    let mut per_file_info: Vec<Option<PerFileBatch>> = Vec::with_capacity(files.len());
    let mut validation_errors: Vec<Option<String>> = vec![None; files.len()];

    for (file_idx, parsed_file) in parsed_files.iter().enumerate() {
        if is_dummy(parsed_file) {
            per_file_info.push(None);
            continue;
        }

        if let Err(errors) =
            validate_to_level(parsed_file, &parse_error_lists[file_idx], hooks.validity)
        {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            let error_summary = format!(
                "{} pre-validation failed: {}",
                hooks.command,
                msgs.join("; ")
            );
            warn!(
                filename = %files[file_idx].filename,
                errors = %error_summary,
                chat_text = %files[file_idx].chat_text,
                command = hooks.command,
                "pre-validation failed — dumping CHAT for diagnosis"
            );
            validation_errors[file_idx] = Some(error_summary);
            per_file_info.push(None);
            continue;
        }

        let batch_items = (hooks.collect)(parsed_file);
        if batch_items.is_empty() {
            per_file_info.push(None);
            continue;
        }

        let global_start = all_items.len();
        let item_count = batch_items.len();
        per_file_info.push(Some(PerFileBatch {
            item_count,
            global_start,
        }));
        all_items.extend(batch_items);
    }

    // 3. Single batch_infer across all files' pooled payloads.
    let all_responses = if all_items.is_empty() {
        Vec::new()
    } else {
        match infer(pool, &all_items, lang).await {
            Ok(responses) => responses,
            Err(e) => {
                warn!(error = %e, command = hooks.command, "Batch infer failed for all files");
                for (file_idx, file) in files.iter().enumerate() {
                    if per_file_info
                        .get(file_idx)
                        .and_then(|f| f.as_ref())
                        .is_some()
                    {
                        results.push(TextBatchFileResult::err(
                            file.filename.clone(),
                            format!("Batch infer failed: {e}"),
                        ));
                    } else {
                        // No payloads collected (empty/dummy); serialize as-is.
                        let chat_file = &mut parsed_files[file_idx];
                        results.push(TextBatchFileResult::ok(
                            file.filename.clone(),
                            to_chat_string(chat_file),
                        ));
                    }
                }
                return results;
            }
        }
    };

    // 4. Redistribute responses per file, apply, post-validate, serialize.
    for (file_idx, file) in files.iter().enumerate() {
        if let Some(ref err) = validation_errors[file_idx] {
            results.push(TextBatchFileResult::err(file.filename.clone(), err.clone()));
            continue;
        }

        let chat_file = &mut parsed_files[file_idx];

        if let Some(ref fm) = per_file_info[file_idx] {
            let end = fm.global_start + fm.item_count;
            let file_items = &all_items[fm.global_start..end];
            let file_responses = &all_responses[fm.global_start..end];
            (hooks.apply)(chat_file, file_items, file_responses);
        }

        if let Err(errors) = validate_output(chat_file, hooks.command) {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            warn!(
                filename = %file.filename.as_ref(),
                command = hooks.command,
                errors = ?msgs,
                "post-validation warnings (non-fatal)"
            );
        }

        results.push(TextBatchFileResult::ok(
            file.filename.clone(),
            to_chat_string(chat_file),
        ));
    }

    results
}
