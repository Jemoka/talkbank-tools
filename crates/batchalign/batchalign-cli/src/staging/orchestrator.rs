//! Staged remote execution orchestrator.
//!
//! Runs as an async task on the main server runtime (via
//! `RuntimeSupervisor::spawn_job`). Manages the full lifecycle:
//!
//! ```text
//! prepare → stage (rsync) → submit → poll → copy-back → done
//! ```
//!
//! Updates the local `JobStore` at each stage so the dashboard shows
//! progress. Forwards cancel signals to the remote server.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use batchalign_types::paths::ClientPath;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::api::{CancelReason, CancelSource, CancellationRequest, JobId, JobInfo, JobStatus};
use crate::config::FleetTarget;
use crate::store::JobStore;
use crate::types::execution_plan::{ExecutionMode, ExecutionPlan, ExecutionStage};
use crate::types::request::JobSubmission;

use super::prepare::prepare_staging_dir;
use super::rsync::{copy_back_results, stage_inputs};

/// Run a staged remote job from start to finish.
///
/// This function is the async task body spawned by
/// `RuntimeSupervisor::spawn_job()`. It:
///
/// 1. Prepares a local staging directory (CHAT + media)
/// 2. Rsyncs it to the remote host
/// 3. Submits a paths-mode job to the remote server
/// 4. Polls the remote server for completion
/// 5. Copies results back to local output paths
/// 6. Updates the local job store at each stage
///
/// On cancellation, sends a cancel request to the remote server.
///
/// `#[allow(clippy::too_many_arguments)]` — pure spawn shim for
/// `RuntimeSupervisor::spawn_job()`. Every argument is owned context
/// that `run_inner` needs by reference; bundling into a struct would
/// only restate the same list at a new name.
#[allow(clippy::too_many_arguments)]
pub async fn run_staged_remote_job(
    store: Arc<JobStore>,
    job_id: JobId,
    config: FleetTarget,
    source_paths: Vec<PathBuf>,
    output_dir: PathBuf,
    jobs_dir: PathBuf,
    hostname: String,
    cancel_token: CancellationToken,
    submission_template: JobSubmission,
) {
    let result = run_inner(
        &store,
        &job_id,
        &config,
        &source_paths,
        &output_dir,
        &jobs_dir,
        &hostname,
        &cancel_token,
        &submission_template,
    )
    .await;

    if let Err(e) = result {
        error!(job_id = %job_id, error = %e, "Staged remote job failed");
        store
            .update_job_status(&job_id, JobStatus::Failed, Some(e.to_string()))
            .await;
    }
}

/// Inner implementation with Result return for clean error handling.
///
/// `#[allow(clippy::too_many_arguments)]` — mirrors `run_staged_remote_job`'s
/// parameter list by reference; splitting into a struct here without also
/// splitting the outer spawn shim would move the lint, not resolve it.
#[allow(clippy::too_many_arguments)]
async fn run_inner(
    store: &Arc<JobStore>,
    job_id: &JobId,
    config: &FleetTarget,
    source_paths: &[PathBuf],
    output_dir: &Path,
    jobs_dir: &Path,
    hostname: &str,
    cancel_token: &CancellationToken,
    submission_template: &JobSubmission,
) -> Result<(), StagedRemoteError> {
    let http = reqwest::Client::new();

    // ── Stage 1: Prepare local staging directory ─────────────────────
    set_plan(store, job_id, ExecutionStage::Staging, None, config).await;

    if cancel_token.is_cancelled() {
        return do_cancel(store, job_id, &http, config, None).await;
    }

    let staging_dir = prepare_staging_dir(job_id, source_paths, jobs_dir)
        .await
        .map_err(StagedRemoteError::Staging)?;

    // ── Stage 2: Rsync to remote ─────────────────────────────────────
    if cancel_token.is_cancelled() {
        return do_cancel(store, job_id, &http, config, None).await;
    }

    let staging_result = stage_inputs(config, hostname, job_id, &staging_dir)
        .await
        .map_err(StagedRemoteError::Staging)?;

    // ── Stage 3: Submit remote job ───────────────────────────────────
    set_plan(store, job_id, ExecutionStage::Executing, None, config).await;

    if cancel_token.is_cancelled() {
        return do_cancel(store, job_id, &http, config, None).await;
    }

    // Build paths-mode submission pointing at remote scratch
    let remote_source_paths: Vec<ClientPath> = source_paths
        .iter()
        .filter_map(|p| p.file_name())
        .map(|name| {
            ClientPath::from(format!(
                "{}{}",
                staging_result.remote_input_dir.display(),
                name.to_string_lossy()
            ))
        })
        .collect();

    let remote_output_paths: Vec<ClientPath> = source_paths
        .iter()
        .filter_map(|p| p.file_name())
        .map(|name| {
            ClientPath::from(format!(
                "{}{}",
                staging_result.remote_output_dir.display(),
                name.to_string_lossy()
            ))
        })
        .collect();

    let remote_submission = JobSubmission {
        command: submission_template.command,
        lang: submission_template.lang.clone(),
        num_speakers: submission_template.num_speakers,
        options: submission_template.options.clone(),
        paths_mode: true,
        source_paths: remote_source_paths,
        output_paths: remote_output_paths,
        source_dir: ClientPath::from(staging_result.remote_input_dir.display().to_string()),
        debug_traces: false,
        // Content fields not used in paths mode
        files: Vec::new(),
        media_files: Vec::new(),
        display_names: Vec::new(),
        before_paths: Vec::new(),
        media_mapping: Default::default(),
        media_subdir: Default::default(),
    };

    let submit_resp = http
        .post(format!("{}/jobs", config.url))
        .json(&remote_submission)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| StagedRemoteError::RemoteSubmit(e.to_string()))?;

    if !submit_resp.status().is_success() {
        let body = submit_resp
            .text()
            .await
            .unwrap_or_else(|e| format!("<body read failed: {e}>"));
        return Err(StagedRemoteError::RemoteSubmit(format!(
            "remote server returned error: {body}"
        )));
    }

    let remote_job: JobInfo = submit_resp
        .json()
        .await
        .map_err(|e| StagedRemoteError::RemoteSubmit(e.to_string()))?;

    let remote_job_id = remote_job.job_id;

    info!(job_id = %job_id, remote_job_id = %remote_job_id, "Remote job submitted");

    set_plan(
        store,
        job_id,
        ExecutionStage::Executing,
        Some(remote_job_id.clone()),
        config,
    )
    .await;

    // ── Stage 4: Poll for remote completion ──────────────────────────
    let mut interval = tokio::time::interval(Duration::from_secs(3));

    loop {
        interval.tick().await;

        if cancel_token.is_cancelled() {
            return do_cancel(store, job_id, &http, config, Some(&remote_job_id)).await;
        }

        let poll_url = format!("{}/jobs/{}", config.url, remote_job_id);
        let resp = match http
            .get(&poll_url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                warn!(status = %r.status(), "Remote poll non-success, retrying");
                continue;
            }
            Err(e) => {
                warn!(error = %e, "Remote poll failed, retrying");
                continue;
            }
        };

        let remote_info: JobInfo = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "Failed to parse remote poll response");
                continue;
            }
        };

        if remote_info.status == JobStatus::Completed {
            info!(job_id = %job_id, remote_job_id = %remote_job_id, "Remote completed");
            break;
        } else if remote_info.status == JobStatus::Failed {
            let err = remote_info.error.as_deref().unwrap_or("unknown error");
            return Err(StagedRemoteError::RemoteExecution(format!(
                "remote job failed: {err}"
            )));
        } else if remote_info.status == JobStatus::Cancelled {
            return Err(StagedRemoteError::RemoteExecution(
                "remote job was cancelled".into(),
            ));
        }
    }

    // ── Stage 5: Copy results back ───────────────────────────────────
    set_plan(
        store,
        job_id,
        ExecutionStage::CopyingBack,
        Some(remote_job_id.clone()),
        config,
    )
    .await;

    match copy_back_results(config, hostname, job_id, output_dir).await {
        Ok(()) => {
            set_plan(
                store,
                job_id,
                ExecutionStage::Done,
                Some(remote_job_id),
                config,
            )
            .await;
            store
                .update_job_status(job_id, JobStatus::Completed, None)
                .await;
            Ok(())
        }
        Err(e) => {
            warn!(
                job_id = %job_id,
                error = %e,
                "Copy-back failed — remote results still on execution host"
            );
            set_plan(
                store,
                job_id,
                ExecutionStage::Failed,
                Some(remote_job_id),
                config,
            )
            .await;
            store
                .update_job_status(
                    job_id,
                    JobStatus::WritebackFailed,
                    Some(format!("copy-back failed: {e}")),
                )
                .await;
            Ok(()) // WritebackFailed is the terminal state, not an error
        }
    }
}

/// Update the execution plan on the local job.
async fn set_plan(
    store: &Arc<JobStore>,
    job_id: &JobId,
    stage: ExecutionStage,
    remote_job_id: Option<JobId>,
    config: &FleetTarget,
) {
    store
        .set_execution_plan(
            job_id,
            Some(ExecutionPlan {
                mode: ExecutionMode::StagedRemote,
                execution_host: config.ssh_host.clone(),
                remote_job_id,
                stage,
            }),
        )
        .await;
}

/// Cancel a remote job (if submitted) and mark the local job as cancelled.
async fn do_cancel(
    store: &Arc<JobStore>,
    job_id: &JobId,
    http: &reqwest::Client,
    config: &FleetTarget,
    remote_job_id: Option<&JobId>,
) -> Result<(), StagedRemoteError> {
    if let Some(rid) = remote_job_id {
        let url = format!("{}/jobs/{}/cancel", config.url, rid);
        let _ = http
            .post(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await;
    }
    // Local cancel mirrors the remote cancel that was just forwarded.
    // Source=Staging records that this came from the orchestrator, not
    // the user's TUI.
    let provenance = CancellationRequest {
        source: Some(CancelSource::Staging),
        reason: Some(CancelReason("staged-remote-cancel-mirror".to_string())),
        ..Default::default()
    };
    let _ = store.cancel(job_id, provenance).await;
    Ok(())
}

/// Errors specific to staged remote execution.
#[derive(Debug, thiserror::Error)]
pub enum StagedRemoteError {
    /// Failed during file staging (prepare or rsync).
    #[error("staging failed: {0}")]
    Staging(#[source] super::rsync::StagingError),
    /// Failed to submit to the remote server.
    #[error("remote submission failed: {0}")]
    RemoteSubmit(String),
    /// The remote job failed or was cancelled.
    #[error("remote execution failed: {0}")]
    RemoteExecution(String),
}
