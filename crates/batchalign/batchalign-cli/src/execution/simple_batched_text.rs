use std::future::Future;

use crate::api::LanguageCode3;
use crate::planning;
use crate::runner::DispatchHostContext;
use crate::runner::util::{FileRunTracker, FileStage};
use crate::scheduling::WorkUnitKind;
use crate::store::{RunnerJobSnapshot, unix_now};
use crate::text_batch::TextBatchFileResults;

use super::text_io::{load_text_inputs, write_text_results};

pub(crate) async fn dispatch_simple_batched_text_job<F, Fut>(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    should_merge_abbrev: bool,
    stage: FileStage,
    planning_label: &str,
    missing_artifact_label: &str,
    batch_fn: F,
) -> Result<(), crate::error::ServerError>
where
    F: FnOnce(Vec<crate::text_batch::TextBatchFileInput>, LanguageCode3) -> Fut,
    Fut: Future<Output = TextBatchFileResults>,
{
    let plan = planning::build_job_plan(job).map_err(|error| {
        crate::error::ServerError::Validation(format!("{planning_label} planning failed: {error}"))
    })?;
    let sink = host.sink().clone();
    let started_at = unix_now();

    for file in &job.pending_files {
        FileRunTracker::new(sink.as_ref(), &job.identity.job_id, file.filename.as_ref())
            .begin_first_attempt(WorkUnitKind::BatchInfer, started_at, stage)
            .await;
    }

    let inputs = load_text_inputs(job, host, false).await;
    if inputs.file_texts.is_empty() {
        return Ok(());
    }

    // No silent eng fallback. If the job's lang is Auto/PerFile, the
    // simple batched text dispatch isn't the right path — return a
    // typed error so the caller routes through the per-file dispatch
    // (`execution/translate.rs`, `execution/morphotag/`,
    // `execution/coref.rs`) which resolves language per-file from each
    // CHAT file's `@Languages:` header.
    let lang = job.dispatch.lang.as_resolved().cloned().ok_or_else(|| {
        crate::error::ServerError::Validation(format!(
            "simple batched text dispatch requires a resolved `--lang <iso3>`; got \
             '{}'. PerFile / Auto must use the per-file execution kernel.",
            job.dispatch.lang
        ))
    })?;
    let results = batch_fn(inputs.file_texts, lang).await;
    write_text_results(
        job,
        host,
        &plan,
        results,
        should_merge_abbrev,
        missing_artifact_label,
    )
    .await;
    Ok(())
}
