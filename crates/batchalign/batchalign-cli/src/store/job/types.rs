//! Job struct, associated types, and conflict detection.

use std::collections::{BTreeMap, HashMap};

use batchalign_types::paths::{ClientPath, MediaMappingKey, RepoRelativePath, ServerPath};

use crate::api::{
    ContentType, CorrelationId, DisplayPath, FileProgressStage, JobId, JobStatus, LanguageSpec,
    NodeId, NumSpeakers, ReleasedCommand, UnixTimestamp,
};
use crate::options::CommandOptions;
use crate::types::execution_plan::ExecutionPlan;
use tokio_util::sync::CancellationToken;

use crate::store::{FileResultEntry, FileStatus};

// ---------------------------------------------------------------------------
// Job
// ---------------------------------------------------------------------------

/// All metadata for a single processing job.
///
/// A job progresses through the state machine:
/// `Queued -> Running -> Completed | Failed | Cancelled`.
/// `Interrupted` is a transient state assigned during crash recovery when the
/// server exited while the job was `Running`.  On reload, interrupted jobs are
/// either re-queued (if resumable files remain) or promoted to a terminal state.
///
/// Jobs are created by `JobStore::submit()`, executed by `runner::run_job()`,
/// and persisted to SQLite for crash recovery.
pub struct Job {
    /// Stable identifiers for the job and its correlation context.
    pub identity: JobIdentity,
    /// Immutable dispatch-time command and option configuration.
    pub dispatch: JobDispatchConfig,
    /// Submitter-facing provenance for conflict detection and display.
    pub source: JobSourceContext,
    /// File lists, staging paths, and media-resolution configuration.
    pub filesystem: JobFilesystemConfig,
    /// Mutable execution state updated as files progress.
    pub execution: JobExecutionState,
    /// Scheduling, completion, and lease state for queue coordination.
    pub schedule: JobScheduleState,
    /// In-memory cancellation and runner-claim state.
    pub runtime: JobRuntimeControl,
    /// Optional execution plan describing where and how this job is processed.
    /// Present for staged-remote jobs; `None` for local and direct-mode jobs.
    pub execution_plan: Option<ExecutionPlan>,
}

/// Stable identifiers for one job.
#[derive(Debug, Clone)]
pub struct JobIdentity {
    /// UUID v4 uniquely identifying this job. Immutable after creation.
    pub job_id: JobId,
    /// Client-supplied correlation ID for tracing across services.
    pub correlation_id: CorrelationId,
}

/// Immutable dispatch-time configuration for one job.
#[derive(Debug, Clone)]
pub struct JobDispatchConfig {
    /// The batchalign command to run.
    pub command: ReleasedCommand,
    /// Language specification — may be `Auto` (for ASR auto-detection) or
    /// a resolved ISO 639-3 code.
    pub lang: LanguageSpec,
    /// Expected number of speakers in the audio workflow.
    pub num_speakers: NumSpeakers,
    /// Typed command options captured at submission time.
    pub options: CommandOptions,
    /// Server-internal runtime state for orchestration helpers.
    pub runtime_state: BTreeMap<String, serde_json::Value>,
    /// Whether detailed algorithm traces should be collected.
    pub debug_traces: bool,
}

/// Submitter-facing provenance for one job.
#[derive(Debug, Clone)]
pub struct JobSourceContext {
    /// IP address or hostname of the submitting client.
    pub submitted_by: String,
    /// Human-readable hostname resolved from `submitted_by`.
    pub submitted_by_name: String,
    /// Client-visible source directory used for display and locality hints.
    pub source_dir: ClientPath,
}

/// File lists and storage layout for one job.
#[derive(Debug, Clone)]
pub struct JobFilesystemConfig {
    /// Ordered list of file basenames to process.
    pub filenames: Vec<DisplayPath>,
    /// Parallel CHAT/media markers for [`Self::filenames`].
    pub has_chat: Vec<bool>,
    /// Server-local temporary directory for staged input/output content.
    pub staging_dir: ServerPath,
    /// Whether the job reads and writes directly on the filesystem.
    pub paths_mode: bool,
    /// Absolute source paths parallel to [`Self::filenames`] in paths mode.
    pub source_paths: Vec<ClientPath>,
    /// Absolute output paths parallel to [`Self::filenames`] in paths mode.
    pub output_paths: Vec<ClientPath>,
    /// Optional "before" paths parallel to [`Self::source_paths`].
    pub before_paths: Vec<ClientPath>,
    /// Key into the server's configured media-mapping roots.
    pub media_mapping: MediaMappingKey,
    /// Optional subdirectory within the selected media mapping.
    pub media_subdir: RepoRelativePath,
    /// Client-provided source directory for media locality inference.
    ///
    /// In paths mode, the FA pipeline uses this to auto-detect the media
    /// mapping via [`batchalign_types::paths::infer_media_mapping()`].
    /// Without this field, `--server` jobs from remote clients cannot
    /// resolve media files.
    pub source_dir: ClientPath,
}

/// Mutable execution state for one job.
#[derive(Debug, Clone)]
pub struct JobExecutionState {
    /// Current lifecycle state of the job.
    pub status: JobStatus,
    /// Per-file processing state keyed by filename.
    pub file_statuses: HashMap<String, FileStatus>,
    /// Accumulated per-file result entries.
    pub results: Vec<FileResultEntry>,
    /// Job-level error message for terminal failures.
    pub error: Option<String>,
    /// Count of files that have reached a terminal status.
    pub completed_files: i64,
    /// Per-language-group progress for batched infer jobs.
    /// Updated by the drain task during morphotag/utseg/translate/coref.
    pub batch_progress: Option<crate::runner::util::batch_progress::BatchInferProgress>,
}

/// Current queue lease for one job.
#[derive(Debug, Clone)]
pub struct JobLeaseState {
    /// Node that currently owns the queue lease for this job.
    pub leased_by_node: Option<NodeId>,
    /// When the current lease will expire if not renewed.
    pub expires_at: Option<UnixTimestamp>,
    /// When the current lease was last created or renewed.
    pub heartbeat_at: Option<UnixTimestamp>,
}

/// Scheduling and completion state for one job.
#[derive(Debug, Clone)]
pub struct JobScheduleState {
    /// Unix timestamp when the job was submitted.
    pub submitted_at: UnixTimestamp,
    /// Unix timestamp when the job reached a terminal state.
    pub completed_at: Option<UnixTimestamp>,
    /// Earliest unix timestamp when a deferred queued job should be retried.
    pub next_eligible_at: Option<UnixTimestamp>,
    /// Number of worker processes used for this job once running.
    pub num_workers: Option<i64>,
    /// Current queue lease state.
    pub lease: JobLeaseState,
    /// Most recent cancel attempt's metadata (denormalized from the
    /// `cancellations` audit table). `None` until a cancel arrives.
    /// Projected onto the `JobInfo`'s `last_cancelled_*` fields.
    pub last_cancel: Option<JobLastCancelInfo>,
}

/// Denormalized snapshot of the most recent cancel attempt.
///
/// Mirrors the relevant columns on `jobs.last_cancelled_*` so the
/// in-memory `Job` projection can fill `JobInfo` without a DB JOIN.
#[derive(Debug, Clone)]
pub struct JobLastCancelInfo {
    /// Wall-clock when the cancel arrived at the server.
    pub at: UnixTimestamp,
    /// Wire-format source string (`"tui"`, `"api"`, `"signal"`, ...).
    pub source: String,
    /// Caller-reported host or peer-IP.
    pub host: Option<String>,
    /// Caller-reported reason text.
    pub reason: Option<String>,
}

/// In-memory runtime controls for one job.
pub struct JobRuntimeControl {
    /// Cancellation token checked between files by the runner.
    pub cancel_token: CancellationToken,
    /// Whether the local queue dispatcher already spawned a runner task.
    pub runner_active: bool,
}

/// One file that still requires runner work.
///
/// This is the stable runner-facing replacement for ad hoc
/// `(usize, DisplayPath, bool)` tuples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingJobFile {
    /// Index into the job's parallel path vectors.
    pub file_index: usize,
    /// Logical filename of the work item.
    pub filename: DisplayPath,
    /// Whether the input is CHAT text rather than media.
    pub has_chat: bool,
}

/// Successful result metadata for one completed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletedFileOutput {
    /// Logical result filename returned to clients.
    pub filename: DisplayPath,
    /// MIME-like content type stored with the result.
    pub content_type: ContentType,
}

/// Failure details for one terminal file error.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FileFailureRecord {
    /// Human-readable error message.
    pub message: String,
    /// Broad failure category for grouping and retry policy.
    pub category: crate::scheduling::FailureCategory,
    /// Terminal timestamp for the failed file.
    pub finished_at: UnixTimestamp,
}

/// Retry metadata for one transient file failure.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FileRetryRecord {
    /// Human-readable retry message shown to clients.
    pub message: String,
    /// Broad failure category for the retryable attempt.
    pub category: crate::scheduling::FailureCategory,
    /// Time when the failed attempt finished.
    pub finished_at: UnixTimestamp,
    /// Earliest time when the next attempt may run.
    pub retry_at: UnixTimestamp,
}

/// Ephemeral progress update for one in-flight file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileProgressRecord {
    /// Stable machine-readable stage code.
    pub stage: FileProgressStage,
    /// Current progress counter when available.
    pub current: Option<i64>,
    /// Total progress counter when available.
    pub total: Option<i64>,
}

/// Stable identity fields for a running job.
#[derive(Debug, Clone)]
pub struct RunnerJobIdentity {
    /// Job identifier for logging and downstream APIs.
    pub job_id: JobId,
    /// Correlation identifier for structured logs.
    pub correlation_id: CorrelationId,
}

/// Immutable dispatch-time configuration for one job.
#[derive(Debug, Clone)]
pub struct RunnerDispatchConfig {
    /// Command being executed.
    pub command: ReleasedCommand,
    /// Language specification — may be `Auto` for ASR auto-detection.
    pub lang: LanguageSpec,
    /// Speaker-count hint for audio workflows.
    pub num_speakers: NumSpeakers,
    /// Typed command options captured at submission time.
    pub options: CommandOptions,
    /// Server-internal runtime state for orchestration helpers.
    pub runtime_state: BTreeMap<String, serde_json::Value>,
    /// Whether algorithm traces should be persisted for this job.
    pub debug_traces: bool,
}

/// Filesystem and media-resolution configuration for one job.
#[derive(Debug, Clone)]
pub struct RunnerFilesystemConfig {
    /// Whether the job reads and writes directly on the filesystem.
    pub paths_mode: bool,
    /// Source paths parallel to [`PendingJobFile::file_index`].
    pub source_paths: Vec<ClientPath>,
    /// Output paths parallel to [`PendingJobFile::file_index`].
    pub output_paths: Vec<ClientPath>,
    /// Optional "before" paths parallel to [`PendingJobFile::file_index`].
    pub before_paths: Vec<ClientPath>,
    /// Staging directory for uploaded content mode.
    pub staging_dir: ServerPath,
    /// Media-mapping key for server-side audio lookup.
    pub media_mapping: MediaMappingKey,
    /// Subdirectory within the selected media mapping.
    pub media_subdir: RepoRelativePath,
    /// Client-provided source directory, used for media locality inference.
    ///
    /// The FA pipeline passes this to `infer_media_mapping()` to auto-detect
    /// which media volume and repo-relative subdir to search for audio files.
    pub source_dir: ClientPath,
}

/// Immutable runner-facing snapshot of job state.
///
/// The runner should read one of these projections instead of repeatedly
/// locking the raw job map and reconstructing the same static configuration.
#[derive(Debug, Clone)]
pub struct RunnerJobSnapshot {
    /// Stable identity values for this job.
    pub identity: RunnerJobIdentity,
    /// Dispatch-time configuration for orchestration and worker calls.
    pub dispatch: RunnerDispatchConfig,
    /// Filesystem and media-resolution layout for this job.
    pub filesystem: RunnerFilesystemConfig,
    /// Cancellation token cloned from the live job state.
    pub cancel_token: CancellationToken,
    /// Files that still need processing.
    pub pending_files: Vec<PendingJobFile>,
}

/// Result of recovering a persisted interrupted/running job on startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryDisposition {
    /// The job still has resumable work and was returned to the queue.
    Requeued,
    /// The job had only terminal failures and was promoted to failed.
    Failed,
    /// The job had completed output and was promoted to completed.
    Completed,
}
