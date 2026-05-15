//! Job and file lifecycle status enums.
//!
//! These are re-exported from [`super::api`] for backward compatibility.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Job status
// ---------------------------------------------------------------------------

/// Lifecycle states for a job — mirrors `JobStatus` enum.
///
/// Jobs progress through a linear lifecycle: `Queued -> Running -> {terminal}`.
/// Terminal states (`Completed`, `Failed`, `Cancelled`, `Interrupted`) are
/// permanent and never transition further.  Only `Cancelled` and `Failed` jobs
/// can be restarted (which resets them to `Queued`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is waiting for a worker to become available.  Transitions to
    /// `Running` once a worker is checked out from the pool.
    Queued,
    /// A worker is actively processing the job's files.  Transitions to a
    /// terminal state when all files finish, or when the job is cancelled.
    Running,
    /// All files were processed successfully.  Terminal.
    Completed,
    /// One or more files encountered an unrecoverable error.  Terminal, but
    /// can be restarted — only files that did not complete will be re-queued.
    Failed,
    /// A user explicitly cancelled the job via `DELETE /jobs/{id}`.  Terminal,
    /// but can be restarted.
    Cancelled,
    /// The server shut down (or crashed) while the job was active.  Terminal.
    /// Detected during crash recovery when the SQLite journal contains
    /// non-terminal jobs from a previous run.
    Interrupted,
    /// Remote execution completed successfully but copying results back to
    /// the local output paths failed.  Terminal.  The remote scratch directory
    /// on the execution host still contains the results.
    #[serde(rename = "writeback_failed")]
    WritebackFailed,
}

impl JobStatus {
    /// A terminal job will never change status again.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Failed
                | Self::Cancelled
                | Self::Interrupted
                | Self::WritebackFailed
        )
    }

    /// An active job is either waiting or currently running.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    /// A job can be cancelled if it is still active.
    pub fn can_cancel(self) -> bool {
        self.is_active()
    }

    /// Only cancelled, failed, or writeback-failed jobs can be restarted.
    pub fn can_restart(self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed | Self::WritebackFailed)
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Interrupted => write!(f, "interrupted"),
            Self::WritebackFailed => write!(f, "writeback_failed"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            "writeback_failed" => Ok(Self::WritebackFailed),
            other => Err(format!("unknown JobStatus: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// File status
// ---------------------------------------------------------------------------

/// Per-file lifecycle states within a job.
///
/// Each file in a job tracks its own status independently.  On job restart,
/// only files in resumable states (`Queued`, `Processing`, `Interrupted`) are
/// re-queued; `Done` and `Error` files are left as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum FileStatusKind {
    /// File is waiting to be dispatched to a worker.
    Queued,
    /// File is currently being processed by a worker.
    Processing,
    /// File was processed successfully and its result is available.  Terminal.
    Done,
    /// Processing failed for this file (see `FileStatusEntry.error`).  Terminal.
    Error,
    /// The job was interrupted (server shutdown/crash) while this file was
    /// in-flight.  Resumable on job restart.
    Interrupted,
}

impl FileStatusKind {
    /// A terminal file will not be processed further.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Error)
    }

    /// A resumable file can be reset to queued on job restart.
    pub fn is_resumable(self) -> bool {
        matches!(self, Self::Interrupted | Self::Processing | Self::Queued)
    }
}

impl std::fmt::Display for FileStatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Processing => write!(f, "processing"),
            Self::Done => write!(f, "done"),
            Self::Error => write!(f, "error"),
            Self::Interrupted => write!(f, "interrupted"),
        }
    }
}

impl std::str::FromStr for FileStatusKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "processing" => Ok(Self::Processing),
            "done" => Ok(Self::Done),
            "error" => Ok(Self::Error),
            "interrupted" => Ok(Self::Interrupted),
            other => Err(format!("unknown FileStatusKind: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// File progress stage
// ---------------------------------------------------------------------------

/// Machine-readable sub-stage within a file's processing lifecycle.
///
/// This is more specific than [`FileStatusKind`]. For example, a file may be
/// in the coarse `Processing` status while its current progress stage is
/// `Aligning`, `BuildingChat`, or `RetryScheduled`.
///
/// The API exposes this enum so clients can branch on stable stage codes
/// without parsing human-facing display labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileProgressStage {
    /// Generic processing fallback for commands without a narrower stage.
    Processing,
    /// Initial file read/setup work.
    Reading,
    /// Media discovery or normalization.
    ResolvingAudio,
    /// Utterance-timing recovery before main alignment.
    RecoveringUtteranceTiming,
    /// Fallback timing recovery after an FA error.
    RecoveringTimingFallback,
    /// Main forced-alignment work.
    Aligning,
    /// Main ASR transcription work.
    Transcribing,
    /// Benchmarking pipeline execution.
    Benchmarking,
    /// Cache inspection before a compute-heavy stage.
    CheckingCache,
    /// Applying computed results back into the document.
    ApplyingResults,
    /// Model-output cleanup before downstream document construction.
    PostProcessing,
    /// Building a CHAT document from intermediate utterance state.
    BuildingChat,
    /// Utterance segmentation in the transcribe pipeline.
    SegmentingUtterances,
    /// Morphosyntax enrichment in the transcribe pipeline.
    AnalyzingMorphosyntax,
    /// Final pipeline serialization/finalization.
    Finalizing,
    /// Final write-to-disk or write-to-result stage.
    Writing,
    /// Parsing CHAT files before dispatch.
    Parsing,
    /// Batched morphosyntax analysis.
    Analyzing,
    /// Batched utterance segmentation.
    Segmenting,
    /// Batched translation.
    Translating,
    /// Batched coreference resolution.
    ResolvingCoreference,
    /// Batched transcript/reference comparison.
    Comparing,
    /// File is deferred for a retry after a retryable failure.
    RetryScheduled,
}

impl FileProgressStage {
    /// Return the operator-facing label for this stable stage code.
    ///
    /// The label is intentionally derived from the enum rather than stored as
    /// an independent source of truth. That keeps control-plane logic typed
    /// while still letting UIs render human-friendly text.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Processing => "Processing",
            Self::Reading => "Reading",
            Self::ResolvingAudio => "Resolving audio",
            Self::RecoveringUtteranceTiming => "Recovering utterance timing",
            Self::RecoveringTimingFallback => "Recovering timing (fallback)",
            Self::Aligning => "Aligning",
            Self::Transcribing => "Transcribing",
            Self::Benchmarking => "Benchmarking",
            Self::CheckingCache => "Checking cache",
            Self::ApplyingResults => "Applying results",
            Self::PostProcessing => "Post-processing",
            Self::BuildingChat => "Building CHAT",
            Self::SegmentingUtterances => "Segmenting utterances",
            Self::AnalyzingMorphosyntax => "Analyzing morphosyntax",
            Self::Finalizing => "Finalizing",
            Self::Writing => "Writing",
            Self::Parsing => "Parsing",
            Self::Analyzing => "Analyzing",
            Self::Segmenting => "Segmenting",
            Self::Translating => "Translating",
            Self::ResolvingCoreference => "Resolving coreference",
            Self::Comparing => "Comparing",
            Self::RetryScheduled => "Retry scheduled",
        }
    }
}
