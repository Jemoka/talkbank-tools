//! Batched text NLP dispatch (morphotag, utseg, translate, coref, compare).

use crate::api::{LanguageCode3, ReleasedCommand};
use crate::pipeline::PipelineServices;
use crate::recipe_runner::runtime::{
    ChatOutputTarget, primary_output_artifact, write_chat_output_artifact_with_provenance_gate,
};
use crate::runner::DispatchHostContext;
use crate::scheduling::{FailureCategory, WorkUnitKind};
use crate::text_batch::TextBatchFileInput;
use tracing::warn;

use crate::store::{RunnerJobSnapshot, unix_now};

use super::super::util::{FileRunTracker, FileStage, set_file_progress};
use super::BatchedInferDispatchPlan;

/// Parse CHAT text, apply merge_abbreviations transform, re-serialize.
pub(in crate::runner) fn apply_merge_abbrev(chat_text: &str) -> String {
    let parser = crate::chat_parser();
    let (mut file, _) = talkbank_transform::parse::parse_lenient(&parser, chat_text);
    talkbank_transform::merge_abbreviations(&mut file);
    talkbank_transform::serialize::to_chat_string(&file)
}

/// Dispatch files via the server-side infer path.
///
/// Reads all CHAT files, runs processing in Rust (parse ->
/// cache -> infer -> inject -> serialize), and records results per file.
pub(crate) async fn dispatch_batched_infer(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    services: PipelineServices<'_>,
    plan: BatchedInferDispatchPlan,
) {
    let BatchedInferDispatchPlan {
        kernel_plan,
        tokenization_mode: _,
        multilingual_policy: _,
        should_merge_abbrev,
        mwt: _,
        l2_morphotag: _,
        respect_pos_hints: _,
    } = plan;
    let job_id = &job.identity.job_id;
    let correlation_id = &*job.identity.correlation_id;
    let file_list = &job.pending_files;
    let command = job.dispatch.command;
    // Job-level language for batch dispatch labeling and worker-pool
    // keying. Per-file resolution still happens inside the orchestrator
    // (which derives from each file's `@Languages` header). For
    // `PerFile`/`Auto` jobs there is no honest job-level language to use
    // as a worker-pool label; rather than fabricating `eng` and risking
    // a misrouted worker reuse, refuse to dispatch and surface a typed
    // error. Morphotag/translate/coref are handled by the recipe-driven
    // execution kernel (`execution/translate.rs`,
    // `execution/morphotag/`, `execution/coref.rs`); the legacy
    // batched-infer path here only fires for utseg, which always has a
    // resolved `--lang`.
    let lang: LanguageCode3 = match job.dispatch.lang.as_resolved() {
        Some(code) => code.clone(),
        None => {
            let err_msg = format!(
                "batched-infer dispatch refused: no resolved job-level language \
                 for command '{}' (lang spec: '{}'). Use the recipe execution \
                 kernel for per-file commands; this legacy path requires a \
                 concrete `--lang`.",
                command, job.dispatch.lang
            );
            tracing::warn!(
                job_id = %job_id,
                correlation_id = %correlation_id,
                "{}",
                err_msg,
            );
            host.sink().fail_job(job_id, &err_msg, unix_now()).await;
            return;
        }
    };
    let lang: &LanguageCode3 = &lang;
    debug_assert_eq!(kernel_plan.file_parallelism_hint, 1);

    let started_at = unix_now();
    let sink = host.sink().clone();

    let stage = FileStage::for_batch_command(command);

    // Mark all files as processing, open their batch attempts, and publish the
    // initial stage label.
    for file in file_list {
        let filename = file.filename.as_ref();
        FileRunTracker::new(sink.as_ref(), job_id, filename)
            .begin_first_attempt(WorkUnitKind::BatchInfer, started_at, stage)
            .await;
    }

    // Read all CHAT file contents (and optional "before" texts for incremental)
    let mut file_texts: Vec<TextBatchFileInput> = Vec::with_capacity(file_list.len());
    let mut read_errors: Vec<(usize, String)> = Vec::new();

    for file in file_list {
        let file_index = file.file_index;
        let filename = file.filename.as_ref();
        let lifecycle = FileRunTracker::new(sink.as_ref(), job_id, filename);

        // Transition to Reading while doing I/O so the frontend shows activity.
        lifecycle.stage(FileStage::Reading).await;

        let read_path: std::path::PathBuf =
            if job.filesystem.paths_mode && file_index < job.filesystem.source_paths.len() {
                job.filesystem.source_paths[file_index]
                    .assume_shared_filesystem()
                    .as_path()
                    .to_owned()
            } else {
                job.filesystem
                    .staging_dir
                    .join("input")
                    .join(filename)
                    .as_path()
                    .to_owned()
            };
        match tokio::fs::read_to_string(&read_path).await {
            Ok(content) => {
                file_texts.push(TextBatchFileInput::new(filename.to_string(), content));
            }
            Err(e) => {
                let err_msg = format!("Failed to read input: {e}");
                lifecycle
                    .fail(&err_msg, FailureCategory::InputMissing, unix_now())
                    .await;
                read_errors.push((file_index, filename.to_string()));
            }
        }
    }

    if file_texts.is_empty() {
        return;
    }

    // Publish the batch total so frontends can display "0/N" while inference runs.
    let total_files = file_texts.len() as i64;
    for file in &file_texts {
        set_file_progress(
            sink.as_ref(),
            job_id,
            file.filename.as_ref(),
            stage,
            Some(0),
            Some(total_files),
        )
        .await;
    }

    // Run the appropriate server-side orchestrator
    let results = match command {
        ReleasedCommand::Utseg => {
            crate::utseg::process_utseg_batch(
                &file_texts,
                lang,
                services.pool,
                services.cache,
                services.engine_version,
                job.dispatch.options.utseg_fallback_policy().is_allowed(),
            )
            .await
        }
        ReleasedCommand::Translate => {
            crate::translate::process_translate_batch(
                &file_texts,
                lang,
                services.pool,
                services.cache,
                services.engine_version,
            )
            .await
        }
        ReleasedCommand::Coref => {
            crate::coref::process_coref_batch(&file_texts, lang, services.pool).await
        }
        _ => {
            warn!(command = %command, "Unsupported batched infer command");
            return;
        }
    };

    let finished_at = unix_now();

    // Record results per file, reporting Writing progress as each file completes.
    let total_results = results.len() as i64;
    for (result_idx, file_result) in results.into_iter().enumerate() {
        let filename = file_result.filename;
        let result = file_result.result;
        let lifecycle = FileRunTracker::new(sink.as_ref(), job_id, filename.as_ref());
        // Find the file_index for this filename
        let file_index = file_list
            .iter()
            .find(|file| file.filename == filename)
            .map(|file| file.file_index)
            .unwrap_or(0);

        match result {
            Ok(output_chat) => {
                // Signal the Writing stage with a per-file counter.
                set_file_progress(
                    sink.as_ref(),
                    job_id,
                    filename.as_ref(),
                    FileStage::Writing,
                    Some(result_idx as i64 + 1),
                    Some(total_results),
                )
                .await;

                // Optionally merge abbreviations before writing
                let output_text = if should_merge_abbrev {
                    apply_merge_abbrev(output_chat.as_ref())
                } else {
                    output_chat.into_string()
                };

                // Write output
                let primary_output = primary_output_artifact(command, &filename);

                let target = ChatOutputTarget::new(
                    &job.filesystem,
                    file_index,
                    &primary_output.display_path,
                );
                if let Err(e) =
                    write_chat_output_artifact_with_provenance_gate(&target, &output_text, command)
                        .await
                {
                    warn!(
                        job_id = %job_id,
                        correlation_id = %correlation_id,
                        filename = %filename,
                        error = %e,
                        "Failed to write infer output"
                    );
                }

                lifecycle
                    .complete_with_result(
                        primary_output.display_path.clone(),
                        primary_output.content_type,
                        finished_at,
                    )
                    .await;
            }
            Err(err) => {
                let err_msg = err.into_message();
                lifecycle
                    .fail(&err_msg, FailureCategory::ProviderTerminal, finished_at)
                    .await;
            }
        }
    }
    let _ = correlation_id; // mark used for non-compare paths
}
