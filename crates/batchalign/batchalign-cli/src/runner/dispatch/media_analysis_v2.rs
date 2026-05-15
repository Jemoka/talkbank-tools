//! Per-file Rust-owned V2 dispatch for media-analysis commands.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Semaphore;
use tracing::{error, warn};

use crate::ensure_wav;
use crate::recipe_runner::runtime::{
    ChatOutputTarget, result_display_path_for_command, write_text_output_artifact,
};
use crate::runner::DispatchHostContext;
use crate::runner::util::{
    FileRunTracker, FileStage, FileTaskOutcome, RunnerEventSink, classify_worker_error,
    drain_supervised_file_tasks, is_retryable_worker_failure, spawn_supervised_file_task,
    user_facing_error,
};
use crate::scheduling::{FailureCategory, RetryPolicy, WorkUnitKind};
use crate::store::{PendingJobFile, RunnerJobSnapshot, unix_now};
use crate::types::worker_v2::{AvqiResultV2, ExecuteOutcomeV2, OpenSmileResultV2, TaskResultV2};
use crate::worker::artifacts_v2::PreparedArtifactRuntimeV2;
use crate::worker::avqi_request_v2::{
    AvqiBuildInputV2, PreparedAvqiRequestIdsV2, build_avqi_request_v2,
};
use crate::worker::opensmile_request_v2::{
    OpenSmileBuildInputV2, PreparedOpenSmileRequestIdsV2, build_opensmile_request_v2,
};
use crate::worker::pool::WorkerPool;

use crate::api::{ContentType, NumWorkers};

use super::MediaAnalysisDispatchPlan;
use super::asr_media::resolve_paths_mode_or_staging_input;

/// Shared runtime dependencies for top-level media-analysis dispatch.
pub(crate) struct MediaAnalysisDispatchRuntime {
    /// Worker pool used for typed V2 media-analysis requests.
    pub pool: Arc<WorkerPool>,
    /// Maximum number of file tasks to run concurrently for this job.
    pub num_workers: NumWorkers,
}

/// Dispatch per-file media-analysis commands through typed worker protocol V2.
pub(crate) async fn dispatch_media_analysis_v2(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    runtime: MediaAnalysisDispatchRuntime,
    plan: MediaAnalysisDispatchPlan,
) {
    let sink = host.sink().clone();
    let file_parallelism_hint = match &plan {
        MediaAnalysisDispatchPlan::Opensmile { kernel_plan, .. }
        | MediaAnalysisDispatchPlan::Avqi { kernel_plan } => kernel_plan.file_parallelism_hint,
    };
    let file_parallelism = runtime
        .num_workers
        .0
        .max(1)
        .min(file_parallelism_hint.max(1));
    let file_sem = Arc::new(Semaphore::new(file_parallelism));
    let mut tasks = Vec::new();

    for file in &job.pending_files {
        if job.cancel_token.is_cancelled() {
            break;
        }

        let Ok(permit) = file_sem.clone().acquire_owned().await else {
            tracing::warn!("file semaphore closed during shutdown");
            break;
        };
        let sink = sink.clone();
        let pool = runtime.pool.clone();
        let job = job.clone();
        let file = file.clone();
        let filename = file.filename.clone();
        let plan = plan.clone();

        tasks.push(spawn_supervised_file_task(
            filename,
            "media-analysis V2 file task",
            async move {
                let _permit = permit;
                process_one_media_analysis_file_v2(&job, sink.clone(), &pool, &file, &plan).await
            },
        ));
    }

    let abnormal_exits = drain_supervised_file_tasks(
        sink.as_ref(),
        &job.identity.job_id,
        &job.cancel_token,
        tasks,
    )
    .await;
    if abnormal_exits > 0 {
        warn!(
            job_id = %job.identity.job_id,
            abnormal_exits,
            "Supervised media-analysis V2 file tasks exited abnormally"
        );
    }
}

async fn process_one_media_analysis_file_v2(
    job: &RunnerJobSnapshot,
    sink: Arc<dyn RunnerEventSink>,
    pool: &Arc<WorkerPool>,
    file: &PendingJobFile,
    plan: &MediaAnalysisDispatchPlan,
) -> FileTaskOutcome {
    let job_id = &job.identity.job_id;
    let correlation_id = &*job.identity.correlation_id;
    let file_index = file.file_index;
    let filename = file.filename.as_ref();
    let lifecycle = FileRunTracker::new(sink.as_ref(), job_id, filename);
    let started_at = unix_now();

    lifecycle
        .begin_first_attempt(
            WorkUnitKind::FileInfer,
            started_at,
            FileStage::ResolvingAudio,
        )
        .await;

    let original_audio_path =
        resolve_paths_mode_or_staging_input(&job.filesystem, file_index, filename);

    let retry_policy = RetryPolicy::default();
    for attempt_number in 1..=retry_policy.max_attempts {
        if attempt_number > 1 {
            lifecycle
                .restart_attempt(WorkUnitKind::FileInfer, unix_now(), FileStage::Processing)
                .await;
        } else {
            lifecycle.stage(FileStage::Processing).await;
        }

        match dispatch_one_media_analysis_attempt(
            job,
            pool,
            file_index,
            filename,
            &original_audio_path,
            plan,
        )
        .await
        {
            Ok((result_filename, output_text, output_type)) => {
                lifecycle.stage(FileStage::Writing).await;
                let finished_at = unix_now();
                let result_display_path = result_filename.clone().into();
                let target =
                    ChatOutputTarget::new(&job.filesystem, file_index, &result_display_path);
                if let Err(error) = write_text_output_artifact(&target, &output_text).await {
                    let err_msg = format!("Failed to write output for {filename}: {error}");
                    lifecycle
                        .fail(&err_msg, FailureCategory::System, finished_at)
                        .await;
                    return FileTaskOutcome::TerminalStateRecorded;
                }

                lifecycle
                    .complete_with_result(result_filename.clone().into(), output_type, finished_at)
                    .await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
            Err(DispatchFailure::RetryableWorker(error, category)) => {
                let finished_at = unix_now();
                let has_retry_budget = attempt_number < retry_policy.max_attempts;
                if has_retry_budget && is_retryable_worker_failure(category) {
                    let retry_number = attempt_number;
                    let backoff_ms = retry_policy.backoff_for_retry(retry_number);
                    let retry_at =
                        crate::api::UnixTimestamp(finished_at.0 + (backoff_ms.0 as f64 / 1000.0));
                    lifecycle
                        .retry(
                            retry_at,
                            category,
                            &format!("Worker error: {error}; retrying in {backoff_ms} ms"),
                            finished_at,
                        )
                        .await;
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms.0)).await;
                    continue;
                }

                let raw_msg = format!("Worker error: {error}");
                warn!(
                    job_id = %job_id,
                    filename,
                    category = %category,
                    raw_error = %raw_msg,
                    "Media-analysis error (raw)"
                );
                let user_msg = user_facing_error(category, "Analysis", filename, &raw_msg);
                lifecycle.fail(&user_msg, category, finished_at).await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
            Err(DispatchFailure::Terminal(error, category)) => {
                let finished_at = unix_now();
                error!(
                    job_id = %job_id,
                    correlation_id = %correlation_id,
                    filename = %filename,
                    error = %error,
                    "Media-analysis V2 dispatch failed"
                );
                let user_msg = user_facing_error(category, "Analysis", filename, &error);
                lifecycle.fail(&user_msg, category, finished_at).await;
                return FileTaskOutcome::TerminalStateRecorded;
            }
        }
    }

    FileTaskOutcome::MissingTerminalState
}

enum DispatchFailure {
    RetryableWorker(String, FailureCategory),
    Terminal(String, FailureCategory),
}

async fn dispatch_one_media_analysis_attempt(
    job: &RunnerJobSnapshot,
    pool: &Arc<WorkerPool>,
    file_index: usize,
    filename: &str,
    original_audio_path: &Path,
    plan: &MediaAnalysisDispatchPlan,
) -> Result<(String, String, ContentType), DispatchFailure> {
    let audio_path = ensure_wav::ensure_wav(original_audio_path, None)
        .await
        .map_err(|error| {
            DispatchFailure::Terminal(
                format!("Media conversion failed for {filename}: {error}"),
                FailureCategory::Validation,
            )
        })?;

    match plan {
        MediaAnalysisDispatchPlan::Opensmile {
            kernel_plan: _,
            feature_set,
        } => {
            dispatch_opensmile_attempt(job, pool, file_index, filename, &audio_path, feature_set)
                .await
        }
        MediaAnalysisDispatchPlan::Avqi { kernel_plan: _ } => {
            dispatch_avqi_attempt(job, pool, file_index, filename, &audio_path).await
        }
    }
}

async fn dispatch_opensmile_attempt(
    job: &RunnerJobSnapshot,
    pool: &Arc<WorkerPool>,
    file_index: usize,
    filename: &str,
    audio_path: &Path,
    feature_set: &str,
) -> Result<(String, String, ContentType), DispatchFailure> {
    let artifacts = PreparedArtifactRuntimeV2::new("opensmile_v2").map_err(|error| {
        DispatchFailure::Terminal(
            format!("failed to create openSMILE V2 artifact runtime: {error}"),
            FailureCategory::Validation,
        )
    })?;

    let request = build_opensmile_request_v2(
        artifacts.store(),
        OpenSmileBuildInputV2 {
            ids: &PreparedOpenSmileRequestIdsV2::new(
                format!("opensmile-v2-request-{file_index}"),
                format!("opensmile-v2-audio-{file_index}"),
            ),
            audio_path,
            feature_set,
            feature_level: "functionals",
        },
    )
    .await
    .map_err(|error| {
        DispatchFailure::Terminal(
            format!("failed to build openSMILE V2 request: {error}"),
            FailureCategory::Validation,
        )
    })?;

    // Media-analysis (opensmile, avqi) is not language-aware, but
    // `dispatch_execute_v2` still needs a concrete worker-pool key. We
    // refuse to invent one: if the job carries `Auto` / `PerFile`,
    // surface a typed error so the user passes `--lang <iso3>`.
    let pool_key = job.dispatch.lang.as_resolved().cloned().ok_or_else(|| {
        DispatchFailure::Terminal(
            format!(
                "media analysis requires `--lang <iso3>`; got '{}'.",
                job.dispatch.lang
            ),
            FailureCategory::Validation,
        )
    })?;
    let response = pool
        .dispatch_execute_v2(&pool_key, &request)
        .await
        .map_err(|error| {
            DispatchFailure::RetryableWorker(error.to_string(), classify_worker_error(&error))
        })?;

    let result = match response.result {
        Some(TaskResultV2::OpensmileResult(result)) => result,
        Some(other) => {
            return Err(DispatchFailure::Terminal(
                format!("openSMILE V2 returned unexpected payload: {other:?}"),
                FailureCategory::ProviderTerminal,
            ));
        }
        None => {
            return Err(DispatchFailure::Terminal(
                "openSMILE V2 response was missing a result payload".into(),
                FailureCategory::ProviderTerminal,
            ));
        }
    };

    if !matches!(response.outcome, ExecuteOutcomeV2::Success) {
        return Err(DispatchFailure::Terminal(
            format!("openSMILE V2 request failed: {:?}", response.outcome),
            FailureCategory::ProviderTerminal,
        ));
    }
    if !result.success {
        return Err(DispatchFailure::Terminal(
            result
                .error
                .unwrap_or_else(|| "openSMILE V2 runtime failed without detail".into()),
            FailureCategory::ProviderTerminal,
        ));
    }

    Ok((
        opensmile_result_filename(filename),
        format_opensmile_csv(&result),
        ContentType::Csv,
    ))
}

async fn dispatch_avqi_attempt(
    job: &RunnerJobSnapshot,
    pool: &Arc<WorkerPool>,
    file_index: usize,
    filename: &str,
    cs_audio_path: &Path,
) -> Result<(String, String, ContentType), DispatchFailure> {
    let sv_audio_path = resolve_avqi_sv_path(cs_audio_path).ok_or_else(|| {
        DispatchFailure::Terminal(
            format!("AVQI input {filename} is missing a paired .sv. audio file name"),
            FailureCategory::Validation,
        )
    })?;
    let sv_audio_path = ensure_wav::ensure_wav(&sv_audio_path, None)
        .await
        .map_err(|error| {
            DispatchFailure::Terminal(
                format!("Media conversion failed for AVQI pair {filename}: {error}"),
                FailureCategory::Validation,
            )
        })?;

    let artifacts =
        PreparedArtifactRuntimeV2::new(format!("avqi_v2_{file_index}")).map_err(|error| {
            DispatchFailure::Terminal(
                format!("failed to create AVQI V2 artifact runtime: {error}"),
                FailureCategory::Validation,
            )
        })?;
    let request = build_avqi_request_v2(
        artifacts.store(),
        AvqiBuildInputV2 {
            ids: &PreparedAvqiRequestIdsV2::new(
                format!("avqi-v2-request-{file_index}"),
                format!("avqi-v2-cs-{file_index}"),
                format!("avqi-v2-sv-{file_index}"),
            ),
            cs_audio_path,
            sv_audio_path: &sv_audio_path,
        },
    )
    .await
    .map_err(|error| {
        DispatchFailure::Terminal(
            format!("failed to build AVQI V2 request: {error}"),
            FailureCategory::Validation,
        )
    })?;

    // Media-analysis (opensmile, avqi) is not language-aware, but
    // `dispatch_execute_v2` still needs a concrete worker-pool key. We
    // refuse to invent one: if the job carries `Auto` / `PerFile`,
    // surface a typed error so the user passes `--lang <iso3>`.
    let pool_key = job.dispatch.lang.as_resolved().cloned().ok_or_else(|| {
        DispatchFailure::Terminal(
            format!(
                "media analysis requires `--lang <iso3>`; got '{}'.",
                job.dispatch.lang
            ),
            FailureCategory::Validation,
        )
    })?;
    let response = pool
        .dispatch_execute_v2(&pool_key, &request)
        .await
        .map_err(|error| {
            DispatchFailure::RetryableWorker(error.to_string(), classify_worker_error(&error))
        })?;

    let result = match response.result {
        Some(TaskResultV2::AvqiResult(result)) => result,
        Some(other) => {
            return Err(DispatchFailure::Terminal(
                format!("AVQI V2 returned unexpected payload: {other:?}"),
                FailureCategory::ProviderTerminal,
            ));
        }
        None => {
            return Err(DispatchFailure::Terminal(
                "AVQI V2 response was missing a result payload".into(),
                FailureCategory::ProviderTerminal,
            ));
        }
    };

    if !matches!(response.outcome, ExecuteOutcomeV2::Success) {
        return Err(DispatchFailure::Terminal(
            format!("AVQI V2 request failed: {:?}", response.outcome),
            FailureCategory::ProviderTerminal,
        ));
    }
    if !result.success {
        return Err(DispatchFailure::Terminal(
            result
                .error
                .unwrap_or_else(|| "AVQI V2 runtime failed without detail".into()),
            FailureCategory::ProviderTerminal,
        ));
    }

    Ok((
        avqi_result_filename(filename),
        format_avqi_report(&result),
        ContentType::Text,
    ))
}

fn format_opensmile_csv(result: &OpenSmileResultV2) -> String {
    let mut headers = BTreeSet::new();
    for row in &result.rows {
        for key in row.keys() {
            headers.insert(key.clone());
        }
    }
    let ordered_headers: Vec<String> = headers.into_iter().collect();
    let mut lines = Vec::with_capacity(result.rows.len().saturating_add(1));
    lines.push(ordered_headers.join(","));
    for row in &result.rows {
        let line = ordered_headers
            .iter()
            .map(|header| {
                row.get(header)
                    .map(|value| value.to_string())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(",");
        lines.push(line);
    }
    lines.join("\n")
}

fn format_avqi_report(result: &AvqiResultV2) -> String {
    [
        ("avqi", result.avqi),
        ("cpps", result.cpps),
        ("hnr", result.hnr),
        ("shimmer_local", result.shimmer_local),
        ("shimmer_local_db", result.shimmer_local_db),
        ("slope", result.slope),
        ("tilt", result.tilt),
    ]
    .into_iter()
    .map(|(name, value)| format!("{name},{value}"))
    .collect::<Vec<_>>()
    .join("\n")
}

fn opensmile_result_filename(filename: &str) -> String {
    result_display_path_for_command(crate::api::ReleasedCommand::Opensmile, filename).to_string()
}

fn avqi_result_filename(filename: &str) -> String {
    result_display_path_for_command(crate::api::ReleasedCommand::Avqi, filename).to_string()
}

fn resolve_avqi_sv_path(cs_audio_path: &Path) -> Option<PathBuf> {
    let file_name = cs_audio_path.file_name()?.to_string_lossy();
    let lower = file_name.to_ascii_lowercase();
    let idx = lower.find(".cs.")?;
    let replacement = format!("{}.sv.{}", &file_name[..idx], &file_name[idx + 4..]);
    Some(cs_audio_path.with_file_name(replacement))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avqi_pair_resolution_rewrites_cs_to_sv() {
        let path = Path::new("/tmp/sample.cs.wav");
        assert_eq!(
            resolve_avqi_sv_path(path).expect("pair path"),
            PathBuf::from("/tmp/sample.sv.wav")
        );
    }

    #[test]
    fn avqi_output_filename_strips_cs_marker() {
        assert_eq!(avqi_result_filename("sample.cs.wav"), "sample.avqi.txt");
    }

    #[test]
    fn opensmile_output_filename_replaces_extension() {
        assert_eq!(
            opensmile_result_filename("sample.mp3"),
            "sample.opensmile.csv"
        );
    }
}
