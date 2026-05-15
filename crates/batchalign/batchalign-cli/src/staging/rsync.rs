//! rsync-over-SSH transport for staging files to/from a remote host.
//!
//! All fleet machines connect via Tailscale, so `rsync -e ssh` works with
//! Tailscale hostnames (e.g. `operator@server`). The transport handles:
//!
//! - Large media files (multi-GB video) via rsync's delta transfer
//! - Resume after interruption (rsync handles file-level resume)
//! - Progress logging via stderr streaming
//!
//! # Error handling
//!
//! rsync exit codes are mapped to [`StagingError`] variants. Non-zero exit
//! always fails the staging step — there is no retry at this level. The
//! orchestrator decides whether to retry or fail the job.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;
use tracing::{info, warn};

use crate::api::JobId;
use crate::config::FleetTarget;

/// Result of a successful input staging operation.
#[derive(Debug, Clone)]
pub struct RemoteStagingResult {
    /// Absolute path to the staged input directory on the remote host.
    pub remote_input_dir: PathBuf,
    /// Absolute path to the (initially empty) output directory on the remote host.
    pub remote_output_dir: PathBuf,
}

/// Errors that can occur during rsync staging or copy-back.
#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    /// rsync process exited with a non-zero code.
    #[error("rsync failed (exit code {exit_code}): {stderr}")]
    RsyncFailed {
        /// rsync exit code.
        exit_code: i32,
        /// Captured stderr from the rsync process.
        stderr: String,
    },
    /// Failed to spawn the rsync process.
    #[error("failed to spawn rsync: {0}")]
    SpawnFailed(#[source] std::io::Error),
    /// The remote execution config is incomplete.
    #[error("remote execution config incomplete: {0}")]
    ConfigIncomplete(String),
    /// Local I/O error (e.g. creating directories).
    #[error("local I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Stage local input files to the remote host's scratch directory via rsync.
///
/// Creates the remote scratch directory structure:
/// ```text
/// <scratch_base>/<submitter>/<job_id>/
///   input/    ← staged files go here
///   output/   ← remote execution writes here
/// ```
///
/// The `local_staging_dir` should already contain all CHAT files and resolved
/// media files ready to transfer.
pub async fn stage_inputs(
    config: &FleetTarget,
    submitter: &str,
    job_id: &JobId,
    local_staging_dir: &Path,
) -> Result<RemoteStagingResult, StagingError> {
    validate_config(config)?;

    let remote_base = format!(
        "{scratch}/{submitter}/{job_id}",
        scratch = config.scratch_base,
        submitter = submitter,
        job_id = job_id,
    );
    let remote_input = format!("{remote_base}/input/");
    let remote_output = format!("{remote_base}/output/");
    let remote_host = format!("{}@{}", config.ssh_user, config.ssh_host);

    // Create remote directories
    info!(
        remote_base = %remote_base,
        submitter = %submitter,
        job_id = %job_id,
        "Creating remote scratch directories"
    );

    let mkdir_output = Command::new("ssh")
        .args([
            &remote_host,
            &format!("mkdir -p {remote_input} {remote_output}"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(StagingError::SpawnFailed)?;

    if !mkdir_output.status.success() {
        let stderr = String::from_utf8_lossy(&mkdir_output.stderr).to_string();
        return Err(StagingError::RsyncFailed {
            exit_code: mkdir_output.status.code().unwrap_or(-1),
            stderr: format!("ssh mkdir failed: {stderr}"),
        });
    }

    // rsync local staging dir to remote input
    let local_src = format!("{}/", local_staging_dir.display());
    let remote_dest = format!("{remote_host}:{remote_input}");

    info!(
        local_src = %local_src,
        remote_dest = %remote_dest,
        "Staging inputs via rsync"
    );

    let rsync_output = Command::new("rsync")
        .args(["-avz", "--progress", &local_src, &remote_dest])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(StagingError::SpawnFailed)?;

    if !rsync_output.status.success() {
        let stderr = String::from_utf8_lossy(&rsync_output.stderr).to_string();
        return Err(StagingError::RsyncFailed {
            exit_code: rsync_output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    let bytes_transferred = rsync_output.stdout.len();
    info!(
        bytes_transferred = bytes_transferred,
        "Input staging complete"
    );

    Ok(RemoteStagingResult {
        remote_input_dir: PathBuf::from(&remote_input),
        remote_output_dir: PathBuf::from(&remote_output),
    })
}

/// Copy results from the remote host's scratch output directory back to a
/// local directory via rsync.
///
/// Called after remote execution completes successfully. The local output
/// directory is created if it doesn't exist.
pub async fn copy_back_results(
    config: &FleetTarget,
    submitter: &str,
    job_id: &JobId,
    local_output_dir: &Path,
) -> Result<(), StagingError> {
    validate_config(config)?;

    let remote_output = format!(
        "{scratch}/{submitter}/{job_id}/output/",
        scratch = config.scratch_base,
        submitter = submitter,
        job_id = job_id,
    );
    let remote_host = format!("{}@{}", config.ssh_user, config.ssh_host);
    let remote_src = format!("{remote_host}:{remote_output}");
    let local_dest = format!("{}/", local_output_dir.display());

    // Ensure local output directory exists
    tokio::fs::create_dir_all(local_output_dir).await?;

    info!(
        remote_src = %remote_src,
        local_dest = %local_dest,
        "Copying results back via rsync"
    );

    let rsync_output = Command::new("rsync")
        .args(["-avz", "--progress", &remote_src, &local_dest])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(StagingError::SpawnFailed)?;

    if !rsync_output.status.success() {
        let stderr = String::from_utf8_lossy(&rsync_output.stderr).to_string();
        warn!(
            exit_code = rsync_output.status.code().unwrap_or(-1),
            stderr = %stderr,
            "Result copy-back failed"
        );
        return Err(StagingError::RsyncFailed {
            exit_code: rsync_output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    info!("Result copy-back complete");
    Ok(())
}

/// Validate that the remote execution config has all required fields.
fn validate_config(config: &FleetTarget) -> Result<(), StagingError> {
    if config.ssh_host.is_empty() {
        return Err(StagingError::ConfigIncomplete(
            "target hostname is empty".to_string(),
        ));
    }
    if config.ssh_user.is_empty() {
        return Err(StagingError::ConfigIncomplete(
            "ssh_user is empty".to_string(),
        ));
    }
    if config.scratch_base.is_empty() {
        return Err(StagingError::ConfigIncomplete(
            "scratch_base is empty".to_string(),
        ));
    }
    Ok(())
}
