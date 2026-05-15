//! Morphotag dispatch — per-file fanout, mirroring the FA / media-analysis
//! pattern (`runner/dispatch/fa_pipeline.rs`,
//! `runner/dispatch/media_analysis_v2.rs`).
//!
//! Each input file is processed independently in its own spawned task,
//! bounded by `Semaphore(num_workers)`. `num_workers` is the
//! existing per-job concurrency cap derived from host capability (see
//! `JobDispatchRequest::num_workers` in `runner/routing.rs`); this is
//! the same memory-aware budget align/transcribe/media-analysis use to
//! avoid the BA2 over-parallelism crash mode.
//!
//! Per-file durability: each task's result is written back to disk
//! (`write_morphotag_results` → `text_io::write_text_results`) as soon
//! as it completes, so a daemon redeploy mid-run loses at most the
//! files currently in flight rather than the entire batch. Rationale
//! recorded in
//! `docs/investigations/2026-04-25-ba3-feedback-by-default-architecture.md`
//! §4.4 and `docs/session-handoff-2026-05-02.md` §6.4.

use std::sync::Arc;

use crate::api::NumWorkers;
use crate::planning;
use crate::runner::DispatchHostContext;
use crate::runner::util::{FileRunTracker, FileStage};
use crate::scheduling::WorkUnitKind;
use crate::store::{RunnerJobSnapshot, unix_now};
use crate::text_batch::TextBatchFileResult;

use super::worker_gateway::{MorphotagRuntimeOptions, WorkerGateway};

mod input;
mod writeback;

use input::load_morphotag_inputs;
use writeback::write_morphotag_results;

/// Dispatch a morphotag job: fan files out across at most `num_workers`
/// concurrent tasks, each invoking the worker pool's per-file morphotag
/// entry and writing its result independently.
pub(crate) async fn dispatch_morphotag_job(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    gateway: Arc<dyn WorkerGateway>,
    options: MorphotagRuntimeOptions,
    num_workers: NumWorkers,
) -> Result<(), crate::error::ServerError> {
    let plan = planning::build_job_plan(job).map_err(|error| {
        crate::error::ServerError::Validation(format!("Morphotag planning failed: {error}"))
    })?;
    let plan = Arc::new(plan);
    let sink = host.sink().clone();

    let inputs = load_morphotag_inputs(job, host).await;
    if inputs.file_texts.is_empty() {
        return Ok(());
    }

    // No job-level lang for morphotag — language is resolved per-file
    // from each CHAT file's `@Languages:` header inside the spawned task.
    // The previous job-level `resolved_lang(job)` lookup silently fell
    // back to English on PerFile (the 2026-05-03 incident shape).
    let file_parallelism = num_workers.0.max(1);
    let file_sem = Arc::new(tokio::sync::Semaphore::new(file_parallelism));
    let mut joinset: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
    let job_id = job.identity.job_id.clone();

    for file_input in inputs.file_texts {
        if job.cancel_token.is_cancelled() {
            break;
        }
        let permit = match file_sem.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                tracing::warn!("morphotag file semaphore closed during shutdown");
                break;
            }
        };

        let gateway_for_task = Arc::clone(&gateway);
        let options_for_task = options.clone();
        let host_for_task = host.clone();
        let job_for_task = job.clone();
        let plan_for_task = Arc::clone(&plan);
        let sink_for_task = Arc::clone(&sink);
        let job_id_for_task = job_id.clone();
        let before_text = inputs
            .before_texts
            .get(file_input.filename.as_ref())
            .cloned();

        joinset.spawn(async move {
            let _permit = permit;

            // Mark this file Analyzing only AFTER its semaphore permit is
            // held — i.e. exactly when a worker slot is actually busy with
            // this file. The previous version of this loop pre-marked every
            // pending file as `processing` upfront, which made `file_statuses`
            // claim 38 000+ files were running on 8 workers and gave every
            // file the same `started_at` (= job-submit time), useless for
            // per-file timing or ETA. FA and media-analysis-v2 follow this
            // same in-task pattern: see `runner/dispatch/fa_pipeline.rs`
            // around the `FileRunTracker::new` + `begin_first_attempt`
            // sequence inside `process_one_align_file`, and
            // `runner/dispatch/media_analysis_v2.rs:116-125` for the
            // media-analysis equivalent.
            let lifecycle = FileRunTracker::new(
                sink_for_task.as_ref(),
                &job_id_for_task,
                file_input.filename.as_ref(),
            );
            lifecycle
                .begin_first_attempt(WorkUnitKind::BatchInfer, unix_now(), FileStage::Analyzing)
                .await;

            // Resolve language per-file from the CHAT file's own
            // `@Languages:` header. No job-level lang, no eng fallback —
            // a missing or malformed header surfaces as a typed file-level
            // error in the job's status.
            let parser = crate::chat_parser();
            let (parsed_chat, _parse_errors) =
                talkbank_transform::parse::parse_lenient(&parser, file_input.chat_text.as_ref());
            let file_lang = match crate::pipeline::morphosyntax::resolve_per_file_lang(&parsed_chat)
            {
                Ok(code) => code,
                Err(err) => {
                    let file_result =
                        TextBatchFileResult::err(file_input.filename.clone(), err.to_string());
                    write_morphotag_results(
                        &job_for_task,
                        &host_for_task,
                        &plan_for_task,
                        vec![file_result],
                        options_for_task.should_merge_abbrev,
                    )
                    .await;
                    lifecycle
                        .fail(
                            &err.to_string(),
                            crate::scheduling::FailureCategory::Validation,
                            unix_now(),
                        )
                        .await;
                    return;
                }
            };

            let result = gateway_for_task
                .morphotag_single(
                    &file_input.chat_text,
                    before_text.as_deref(),
                    &file_lang,
                    options_for_task.clone(),
                )
                .await;
            let file_result = match result {
                Ok(text) => TextBatchFileResult::ok(file_input.filename.clone(), text),
                Err(error) => {
                    TextBatchFileResult::err(file_input.filename.clone(), error.to_string())
                }
            };
            write_morphotag_results(
                &job_for_task,
                &host_for_task,
                &plan_for_task,
                vec![file_result],
                options_for_task.should_merge_abbrev,
            )
            .await;
        });
    }

    while let Some(join_result) = joinset.join_next().await {
        if let Err(error) = join_result {
            tracing::warn!(
                job_id = %job.identity.job_id,
                error = %error,
                "Morphotag per-file task panicked"
            );
        }
    }

    Ok(())
}
