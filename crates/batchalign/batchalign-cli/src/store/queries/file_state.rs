//! File-level job-state mutations on [`JobStore`].

use crate::api::{DisplayPath, FileStatusKind, JobId, UnixTimestamp};
use crate::scheduling::{AttemptOutcome, RetryDisposition, WorkUnitKind};

use super::super::{
    AttemptFinishRecord, AttemptStartRecord, CompletedFileOutput, FileFailureRecord,
    FileProgressRecord, FileRetryRecord, JobStore, PersistedFileUpdate,
};

impl JobStore {
    /// Mark one file as processing and persist the start timestamp.
    pub(crate) async fn mark_file_processing(
        &self,
        job_id: &JobId,
        filename: &str,
        started_at: UnixTimestamp,
    ) {
        let Some(update) = self
            .registry
            .mark_file_processing(job_id, filename, started_at)
            .await
        else {
            return;
        };
        self.notify_file_update(&update.job_id, update.file, update.completed_files);

        self.db_update_file(
            job_id,
            PersistedFileUpdate {
                filename,
                status: FileStatusKind::Processing,
                error: None,
                error_category: None,
                bug_report_id: None,
                content_type: None,
                started_at: Some(started_at),
                finished_at: None,
                next_eligible_at: None,
            },
        )
        .await;
    }

    /// Mark one file as done and optionally record a downloadable result.
    pub(crate) async fn mark_file_done(
        &self,
        job_id: &JobId,
        filename: &str,
        finished_at: UnixTimestamp,
        result: Option<CompletedFileOutput>,
    ) {
        let persisted_content_type: Option<String> = result
            .as_ref()
            .map(|output| output.content_type.to_string());

        let Some(update) = self
            .registry
            .mark_file_done(job_id, filename, finished_at, result)
            .await
        else {
            return;
        };
        self.notify_file_update(&update.job_id, update.file, update.completed_files);

        self.db_update_file(
            job_id,
            PersistedFileUpdate {
                filename,
                status: FileStatusKind::Done,
                error: None,
                error_category: None,
                bug_report_id: None,
                content_type: persisted_content_type.as_deref(),
                started_at: None,
                finished_at: Some(finished_at),
                next_eligible_at: None,
            },
        )
        .await;
    }

    /// Mark one file as terminally failed and record the error result.
    pub(crate) async fn mark_file_error(
        &self,
        job_id: &JobId,
        filename: &str,
        failure: &FileFailureRecord,
    ) {
        let Some(update) = self
            .registry
            .mark_file_error(job_id, filename, failure)
            .await
        else {
            return;
        };
        self.notify_file_update(&update.job_id, update.file, update.completed_files);

        let error_category = failure.category.to_string();
        self.db_update_file(
            job_id,
            PersistedFileUpdate {
                filename,
                status: FileStatusKind::Error,
                error: Some(&failure.message),
                error_category: Some(error_category.as_str()),
                bug_report_id: None,
                content_type: None,
                started_at: None,
                finished_at: Some(failure.finished_at),
                next_eligible_at: None,
            },
        )
        .await;
        self.db_finish_attempt_for_file(
            job_id,
            AttemptFinishRecord {
                filename,
                outcome: AttemptOutcome::Failed,
                failure_category: Some(failure.category),
                disposition: RetryDisposition::TerminalFailure,
                finished_at: failure.finished_at,
            },
        )
        .await;
    }

    /// Mark one file attempt as started and attach the new attempt record.
    pub(crate) async fn start_file_attempt(
        &self,
        job_id: &JobId,
        filename: &str,
        work_unit_kind: WorkUnitKind,
        started_at: UnixTimestamp,
    ) {
        let Some(update) = self
            .registry
            .start_file_attempt(job_id, filename, started_at)
            .await
        else {
            return;
        };
        self.notify_file_update(&update.job_id, update.file, update.completed_files);

        self.db_update_file(
            job_id,
            PersistedFileUpdate {
                filename,
                status: FileStatusKind::Processing,
                error: None,
                error_category: None,
                bug_report_id: None,
                content_type: None,
                started_at: Some(started_at),
                finished_at: None,
                next_eligible_at: None,
            },
        )
        .await;
        self.db_start_attempt(
            job_id,
            AttemptStartRecord {
                filename,
                work_unit_kind,
                started_at,
            },
        )
        .await;
        self.bump_counter(|c| c.attempts_started += 1).await;
    }

    /// Mark one file as waiting for a retry after a transient failure.
    pub(crate) async fn mark_file_retry_pending(
        &self,
        job_id: &JobId,
        filename: &str,
        retry: &FileRetryRecord,
    ) {
        let Some(update) = self
            .registry
            .mark_file_retry_pending(job_id, filename, retry)
            .await
        else {
            return;
        };
        self.notify_file_update(&update.job_id, update.file, update.completed_files);

        let error_category = retry.category.to_string();
        self.db_update_file(
            job_id,
            PersistedFileUpdate {
                filename,
                status: FileStatusKind::Processing,
                error: Some(&retry.message),
                error_category: Some(error_category.as_str()),
                bug_report_id: None,
                content_type: None,
                started_at: None,
                finished_at: Some(retry.finished_at),
                next_eligible_at: Some(retry.retry_at),
            },
        )
        .await;
        self.db_finish_attempt_for_file(
            job_id,
            AttemptFinishRecord {
                filename,
                outcome: AttemptOutcome::RetryableFailure,
                failure_category: Some(retry.category),
                disposition: RetryDisposition::Retry,
                finished_at: retry.finished_at,
            },
        )
        .await;
        self.bump_counter(|c| c.attempts_retried += 1).await;
        self.bump_counter(|c| c.deferred_work_units += 1).await;
    }

    /// Clear transient retry state before a new attempt starts or succeeds.
    pub(crate) async fn clear_file_retry_state(&self, job_id: &JobId, filename: &str) {
        let _ = self.registry.clear_file_retry_state(job_id, filename).await;
    }

    /// Apply an ephemeral progress update to one file and notify listeners.
    pub(crate) async fn set_file_progress(
        &self,
        job_id: &JobId,
        filename: &str,
        progress: &FileProgressRecord,
    ) {
        if let Some(update) = self
            .registry
            .set_file_progress(job_id, filename, progress)
            .await
        {
            self.notify_file_update(&update.job_id, update.file, update.completed_files);
        }
    }

    /// Return the filenames of files that have not yet reached a terminal state.
    pub(crate) async fn unfinished_files(&self, job_id: &JobId) -> Vec<DisplayPath> {
        self.registry.unfinished_files(job_id).await
    }

    /// Return the current file-status label for one file.
    pub(crate) async fn file_status_label(&self, job_id: &JobId, filename: &str) -> Option<String> {
        self.registry.file_status_label(job_id, filename).await
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    use crate::api::{ContentType, FileStatusKind, JobId, ReleasedCommand, UnixTimestamp};
    use crate::scheduling::FailureCategory;
    use crate::store::queries::tests::{make_job, test_config};
    use crate::ws::BROADCAST_CAPACITY;

    use super::*;

    /// Completing a file records one result and increments the terminal count.
    #[tokio::test]
    async fn mark_file_done_records_result() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);
        store
            .submit(make_job(
                "job-1",
                ReleasedCommand::Morphotag,
                vec!["a.cha".into()],
            ))
            .await
            .unwrap();

        store
            .mark_file_done(
                &JobId::from("job-1"),
                "a.cha",
                UnixTimestamp(10.0),
                Some(CompletedFileOutput {
                    filename: DisplayPath::from("a.cha"),
                    content_type: ContentType::Chat,
                }),
            )
            .await;

        let detail = store.get_job_detail(&JobId::from("job-1")).await.unwrap();
        assert_eq!(detail.results.len(), 1);
        assert_eq!(detail.results[0].error, None);
    }

    /// Retry-pending updates keep the file in processing state with a deadline.
    #[tokio::test]
    async fn mark_file_retry_pending_sets_deadline() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);
        store
            .submit(make_job(
                "job-1",
                ReleasedCommand::Morphotag,
                vec!["a.cha".into()],
            ))
            .await
            .unwrap();

        store
            .mark_file_retry_pending(
                &JobId::from("job-1"),
                "a.cha",
                &FileRetryRecord {
                    message: "retry later".into(),
                    category: FailureCategory::WorkerTimeout,
                    finished_at: UnixTimestamp(10.0),
                    retry_at: UnixTimestamp(20.0),
                },
            )
            .await;

        let detail = store.get_job_detail(&JobId::from("job-1")).await.unwrap();
        let file = detail
            .file_statuses
            .into_iter()
            .find(|status| status.filename == "a.cha")
            .unwrap();
        assert_eq!(file.status, FileStatusKind::Processing);
        assert_eq!(file.next_eligible_at, Some(UnixTimestamp(20.0)));
    }
}
