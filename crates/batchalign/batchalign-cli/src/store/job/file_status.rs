//! Per-file status mutation methods.
//!
//! These methods transition individual file entries through their lifecycle:
//! `Queued → Processing → Done | Error`, with retry scheduling as a transient
//! sub-state within `Processing`.  Each method returns `false` if the filename
//! is not found in the job's file status map.

use crate::api::{ContentType, DisplayPath, FileProgressStage, FileStatusKind, UnixTimestamp};
use crate::store::FileResultEntry;

use super::Job;
use super::types::{CompletedFileOutput, FileFailureRecord, FileProgressRecord, FileRetryRecord};

impl Job {
    /// Mark one file as actively processing.
    ///
    /// Entering processing clears any stale retry/error metadata because a new
    /// attempt should present as "currently running", not "running but still
    /// errored from the last attempt".
    pub(crate) fn mark_file_processing(
        &mut self,
        filename: &str,
        started_at: UnixTimestamp,
    ) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.status = FileStatusKind::Processing;
        file_status.error = None;
        file_status.error_category = None;
        file_status.started_at = Some(started_at);
        file_status.finished_at = None;
        file_status.next_eligible_at = None;
        file_status.progress_current = None;
        file_status.progress_total = None;
        file_status.progress_stage = None;
        true
    }

    /// Mark one file as complete and optionally attach a result record.
    ///
    /// This also clears any retry-era error metadata so the completed file
    /// snapshot matches the persisted database row and the operator-facing API.
    pub(crate) fn mark_file_done(
        &mut self,
        filename: &str,
        finished_at: UnixTimestamp,
        result: Option<CompletedFileOutput>,
    ) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.status = FileStatusKind::Done;
        file_status.error = None;
        file_status.error_category = None;
        file_status.finished_at = Some(finished_at);
        file_status.next_eligible_at = None;
        file_status.progress_current = None;
        file_status.progress_total = None;
        file_status.progress_stage = None;
        if let Some(result) = result {
            self.execution.results.push(FileResultEntry {
                filename: result.filename,
                content_type: result.content_type,
                error: None,
            });
        }
        self.execution.completed_files += 1;
        true
    }

    /// Mark one file as terminally failed and attach an error result.
    pub(crate) fn mark_file_error(&mut self, filename: &str, failure: &FileFailureRecord) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.status = FileStatusKind::Error;
        file_status.error = Some(failure.message.clone());
        file_status.error_category = Some(failure.category);
        file_status.finished_at = Some(failure.finished_at);
        file_status.next_eligible_at = None;
        file_status.progress_current = None;
        file_status.progress_total = None;
        file_status.progress_stage = None;
        self.execution.results.push(FileResultEntry {
            filename: DisplayPath::from(filename),
            content_type: ContentType::Chat,
            error: Some(failure.message.clone()),
        });
        self.execution.completed_files += 1;
        true
    }

    /// Record the start of a new file attempt.
    pub(crate) fn start_file_attempt(&mut self, filename: &str, started_at: UnixTimestamp) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.started_at = Some(started_at);
        file_status.finished_at = None;
        file_status.next_eligible_at = None;
        file_status.progress_stage = None;
        true
    }

    /// Mark one file as waiting for a retry after a transient failure.
    pub(crate) fn mark_file_retry_pending(
        &mut self,
        filename: &str,
        retry: &FileRetryRecord,
    ) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.status = FileStatusKind::Processing;
        file_status.error = Some(retry.message.clone());
        file_status.error_category = Some(retry.category);
        file_status.finished_at = Some(retry.finished_at);
        file_status.next_eligible_at = Some(retry.retry_at);
        file_status.progress_current = None;
        file_status.progress_total = None;
        file_status.progress_stage = Some(FileProgressStage::RetryScheduled);
        true
    }

    /// Clear transient retry state before a new attempt starts or succeeds.
    ///
    /// Retry scheduling temporarily stores the last retryable error on the
    /// file so operators can see why the retry was queued. Once a new attempt
    /// starts, that stale retry error must disappear from the live file state
    /// or the dashboard/API will report a successful retry as still errored.
    pub(crate) fn clear_file_retry_state(&mut self, filename: &str) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.error = None;
        file_status.error_category = None;
        file_status.finished_at = None;
        file_status.next_eligible_at = None;
        file_status.progress_stage = None;
        true
    }

    /// Apply an ephemeral progress update to one file.
    pub(crate) fn set_file_progress(
        &mut self,
        filename: &str,
        progress: &FileProgressRecord,
    ) -> bool {
        let Some(file_status) = self.execution.file_statuses.get_mut(filename) else {
            return false;
        };
        file_status.progress_stage = Some(progress.stage);
        file_status.progress_current = progress.current;
        file_status.progress_total = progress.total;
        true
    }

    /// Return the filenames of files that have not yet reached a terminal state.
    pub(crate) fn unfinished_files(&self) -> Vec<DisplayPath> {
        self.execution
            .file_statuses
            .values()
            .filter(|file_status| !file_status.status.is_terminal())
            .map(|file_status| file_status.filename.clone())
            .collect()
    }

    /// Return the current lifecycle label for one file.
    pub(crate) fn file_status_label(&self, filename: &str) -> Option<String> {
        self.execution
            .file_statuses
            .get(filename)
            .map(|file_status| file_status.status.to_string())
    }
}
