//! Execution plan types for job observability.
//!
//! Every job has an optional execution plan that describes where and how it
//! will be processed. This is the transparency layer that lets users and the
//! dashboard see whether a job is running locally, was staged to a remote
//! host, or was submitted explicitly to a remote server.
//!
//! # Modes
//!
//! | Mode | When | Example |
//! |------|------|---------|
//! | [`ExecutionMode::Local`] | No `--server`, local daemon processes | user on local machine |
//! | [`ExecutionMode::StagedRemote`] | No `--server`, local daemon stages to remote | user → remote server |
//! | [`ExecutionMode::ExplicitRemote`] | `--server URL` | user → remote server explicitly |

use serde::{Deserialize, Serialize};

use crate::api::JobId;

/// Execution plan describing where and how a job is processed.
///
/// Attached to `Job` and projected into `JobInfo` for API consumers.
/// The `stage` field tracks progress through the staged-remote lifecycle;
/// for local and explicit-remote jobs it is always [`ExecutionStage::Done`]
/// once the job finishes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct ExecutionPlan {
    /// How the job is being executed.
    pub mode: ExecutionMode,
    /// Hostname of the machine executing the job (e.g. `"server-01"`).
    pub execution_host: String,
    /// For staged-remote jobs: the job ID on the remote server.
    /// `None` until the remote job is submitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_job_id: Option<JobId>,
    /// Current lifecycle stage of the execution plan.
    pub stage: ExecutionStage,
}

/// How a job reaches its execution host.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Processed on the submitting machine's own daemon.
    Local,
    /// Inputs staged via rsync to a remote host, executed there,
    /// results copied back. The user sees one local job.
    StagedRemote,
    /// Submitted directly to a remote server via `--server`.
    ExplicitRemote,
}

/// Lifecycle stage of a staged-remote execution plan.
///
/// For [`ExecutionMode::Local`] and [`ExecutionMode::ExplicitRemote`] jobs,
/// the stage is always [`Done`](ExecutionStage::Done) on completion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStage {
    /// Inputs are being transferred to the remote host via rsync.
    Staging,
    /// The remote server is processing the job.
    Executing,
    /// Remote execution completed; results are being copied back.
    CopyingBack,
    /// Execution plan completed successfully.
    Done,
    /// A stage failed (staging, execution, or copy-back).
    Failed,
}

impl std::fmt::Display for ExecutionStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Staging => "staging",
            Self::Executing => "executing",
            Self::CopyingBack => "copying_back",
            Self::Done => "done",
            Self::Failed => "failed",
        })
    }
}
