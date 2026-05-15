//! Per-file lifecycle tracker and free helper functions for file state
//! mutations.
//!
//! Dispatch code should prefer [`FileRunTracker`] over hand-sequencing raw
//! store mutations. That keeps the per-file state machine explicit.

use crate::api::{ContentType, DisplayPath, JobId, UnixTimestamp};
use crate::scheduling::{AttemptOutcome, FailureCategory, RetryDisposition, WorkUnitKind};
use crate::store::CompletedFileOutput;

use super::{FileStage, RunnerEventSink};

// ---------------------------------------------------------------------------
// Free helper functions for individual file-state mutations
// ---------------------------------------------------------------------------

/// Mark a file as actively processing and persist the start timestamp.
pub(crate) async fn mark_file_processing(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    started_at: UnixTimestamp,
) {
    sink.mark_file_processing(job_id, filename, started_at)
        .await;
}

/// Mark a file as successfully completed and attach one result entry.
pub(crate) async fn mark_file_done(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    result_filename: DisplayPath,
    content_type: ContentType,
    finished_at: UnixTimestamp,
) {
    sink.mark_file_done(
        job_id,
        filename,
        finished_at,
        Some(CompletedFileOutput {
            filename: result_filename,
            content_type,
        }),
    )
    .await;
}

/// Mark a file as done without recording a downloadable result artifact.
pub(crate) async fn mark_file_done_without_result(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    finished_at: UnixTimestamp,
) {
    sink.mark_file_done(job_id, filename, finished_at, None)
        .await;
}

/// Set a file to error status.
pub(crate) async fn set_file_error(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    error: &str,
    category: FailureCategory,
    finished_at: UnixTimestamp,
) {
    sink.mark_file_error(job_id, filename, error, category, finished_at)
        .await;
}

/// Increment the control-plane counter for started work-unit attempts.
pub(crate) async fn start_file_attempt(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    work_unit_kind: WorkUnitKind,
    started_at: UnixTimestamp,
) {
    sink.start_file_attempt(job_id, filename, work_unit_kind, started_at)
        .await;
}

/// Finalize a successful file attempt after the output has been persisted.
pub(crate) async fn finish_file_attempt_success(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    finished_at: UnixTimestamp,
) {
    sink.finish_file_attempt(
        job_id,
        filename,
        AttemptOutcome::Succeeded,
        None,
        RetryDisposition::Succeed,
        finished_at,
    )
    .await;
}

/// Mark a file as waiting for a retry after a transient attempt failure.
pub(crate) async fn set_file_retry_pending(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    retry_at: UnixTimestamp,
    category: FailureCategory,
    message: &str,
    finished_at: UnixTimestamp,
) {
    sink.mark_file_retry_pending(job_id, filename, retry_at, category, message, finished_at)
        .await;
}

/// Clear transient retry state before a new attempt starts or succeeds.
pub(crate) async fn clear_retry_state(sink: &dyn RunnerEventSink, job_id: &JobId, filename: &str) {
    sink.clear_file_retry_state(job_id, filename).await;
}

/// Update ephemeral progress fields on a file and broadcast the update.
///
/// Progress fields are never persisted to SQLite — they are purely for
/// live display in the CLI/TUI/React dashboard.
pub(crate) async fn set_file_progress(
    sink: &dyn RunnerEventSink,
    job_id: &JobId,
    filename: &str,
    stage: FileStage,
    current: Option<i64>,
    total: Option<i64>,
) {
    sink.set_file_progress(job_id, filename, stage, current, total)
        .await;
}

// ---------------------------------------------------------------------------
// FileTaskOutcome — completion contract for supervised file tasks
// ---------------------------------------------------------------------------

/// Explicit completion contract for one supervised file task.
///
/// A task returns `TerminalStateRecorded` only after it has already written the
/// final file status that the runner should trust. Any early return, panic, or
/// cancellation that skips that write path must surface as
/// `MissingTerminalState` so the supervisor can record a concrete failure.
pub(crate) enum FileTaskOutcome {
    /// The task itself recorded success or terminal failure for the file.
    TerminalStateRecorded,
    /// The task exited without recording a terminal file state.
    MissingTerminalState,
}

// ---------------------------------------------------------------------------
// FileRunTracker — per-file lifecycle helper
// ---------------------------------------------------------------------------

/// Runner-side helper for one file's lifecycle and attempt bookkeeping.
///
/// Dispatch code should prefer this helper over hand-sequencing raw store
/// mutations. That keeps the per-file state machine explicit:
///
/// - begin the first processing attempt
/// - move between human-readable stages
/// - restart the attempt after retryable failures
/// - finish as success, retry, or terminal error
pub(crate) struct FileRunTracker<'a> {
    sink: &'a dyn RunnerEventSink,
    job_id: &'a JobId,
    filename: &'a str,
}

impl<'a> FileRunTracker<'a> {
    /// Bind the helper to one `(job_id, filename)` pair.
    pub(crate) fn new(sink: &'a dyn RunnerEventSink, job_id: &'a JobId, filename: &'a str) -> Self {
        Self {
            sink,
            job_id,
            filename,
        }
    }

    /// Mark the file as processing, open the first durable attempt, and set the
    /// initial stage label shown to operators.
    pub(crate) async fn begin_first_attempt(
        &self,
        work_unit_kind: WorkUnitKind,
        started_at: UnixTimestamp,
        stage: FileStage,
    ) {
        mark_file_processing(self.sink, self.job_id, self.filename, started_at).await;
        clear_retry_state(self.sink, self.job_id, self.filename).await;
        start_file_attempt(
            self.sink,
            self.job_id,
            self.filename,
            work_unit_kind,
            started_at,
        )
        .await;
        self.stage(stage).await;
    }

    /// Open a durable setup attempt that fails before the file ever enters the
    /// normal processing pipeline.
    ///
    /// This is used for preflight rejection paths such as missing or
    /// incompatible media where we still want attempt history but should not
    /// advertise the file as actively processing.
    pub(crate) async fn record_setup_failure(
        &self,
        started_at: UnixTimestamp,
        error: &str,
        category: FailureCategory,
        finished_at: UnixTimestamp,
    ) {
        clear_retry_state(self.sink, self.job_id, self.filename).await;
        start_file_attempt(
            self.sink,
            self.job_id,
            self.filename,
            WorkUnitKind::FileSetup,
            started_at,
        )
        .await;
        self.fail(error, category, finished_at).await;
    }

    /// Clear retry-only state, open the next attempt, and publish the stage
    /// label for the new run.
    pub(crate) async fn restart_attempt(
        &self,
        work_unit_kind: WorkUnitKind,
        started_at: UnixTimestamp,
        stage: FileStage,
    ) {
        clear_retry_state(self.sink, self.job_id, self.filename).await;
        start_file_attempt(
            self.sink,
            self.job_id,
            self.filename,
            work_unit_kind,
            started_at,
        )
        .await;
        self.stage(stage).await;
    }

    /// Update the current human-readable progress stage.
    pub(crate) async fn stage(&self, stage: FileStage) {
        set_file_progress(self.sink, self.job_id, self.filename, stage, None, None).await;
    }

    /// Record a retryable failure and publish the retry deadline.
    pub(crate) async fn retry(
        &self,
        retry_at: UnixTimestamp,
        category: FailureCategory,
        message: &str,
        finished_at: UnixTimestamp,
    ) {
        set_file_retry_pending(
            self.sink,
            self.job_id,
            self.filename,
            retry_at,
            category,
            message,
            finished_at,
        )
        .await;
    }

    /// Record a terminal file failure.
    pub(crate) async fn fail(
        &self,
        error: &str,
        category: FailureCategory,
        finished_at: UnixTimestamp,
    ) {
        set_file_error(
            self.sink,
            self.job_id,
            self.filename,
            error,
            category,
            finished_at,
        )
        .await;
    }

    /// Mark the file as done with a downloadable result and close the active
    /// attempt as successful.
    pub(crate) async fn complete_with_result(
        &self,
        result_filename: DisplayPath,
        content_type: ContentType,
        finished_at: UnixTimestamp,
    ) {
        mark_file_done(
            self.sink,
            self.job_id,
            self.filename,
            result_filename,
            content_type,
            finished_at,
        )
        .await;
        finish_file_attempt_success(self.sink, self.job_id, self.filename, finished_at).await;
    }

    /// Mark the file as done without a downloadable artifact and close the
    /// active attempt as successful.
    pub(crate) async fn complete_without_result(&self, finished_at: UnixTimestamp) {
        mark_file_done_without_result(self.sink, self.job_id, self.filename, finished_at).await;
        finish_file_attempt_success(self.sink, self.job_id, self.filename, finished_at).await;
    }
}

// ---------------------------------------------------------------------------
// ProgressUpdate — typed channel messages from orchestrators to dispatch
// ---------------------------------------------------------------------------

/// A progress update from an orchestrator to the dispatch layer.
pub(crate) struct ProgressUpdate {
    /// Typed lifecycle/progress label.
    pub label: FileStage,
    /// Current progress counter (optional).
    pub current: Option<i64>,
    /// Total items for progress (optional).
    pub total: Option<i64>,
}

impl ProgressUpdate {
    /// Construct a typed progress update for the shared file-status channel.
    pub(crate) fn new(label: FileStage, current: Option<i64>, total: Option<i64>) -> Self {
        Self {
            label,
            current,
            total,
        }
    }
}

/// Sender half for progress updates. Orchestrators hold this.
pub(crate) type ProgressSender = tokio::sync::mpsc::UnboundedSender<ProgressUpdate>;
