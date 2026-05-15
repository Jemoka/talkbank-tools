//! Runner-owned boundary for publishing file/job lifecycle events.
//!
//! The execution engine reports what happened through the [`RunnerEventSink`]
//! trait instead of reaching into the concrete store implementation directly.
//! [`StoreRunnerEventSink`] is the production implementation backed by
//! [`JobStore`].

use std::sync::Arc;

use async_trait::async_trait;

use crate::api::{DisplayPath, JobId, JobStatus, UnixTimestamp};
use crate::scheduling::{AttemptOutcome, FailureCategory, RetryDisposition, WorkUnitKind};
use crate::store::{
    AttemptFinishRecord, CompletedFileOutput, FileFailureRecord, FileProgressRecord,
    FileRetryRecord, JobStore,
};

use super::FileStage;

/// Runner-owned boundary for publishing file/job lifecycle events.
///
/// The execution engine should report what happened through this sink instead
/// of reaching into the concrete store implementation directly.
#[async_trait]
pub(crate) trait RunnerEventSink: Send + Sync {
    async fn mark_file_processing(&self, job_id: &JobId, filename: &str, started_at: UnixTimestamp);
    async fn mark_file_done(
        &self,
        job_id: &JobId,
        filename: &str,
        finished_at: UnixTimestamp,
        result: Option<CompletedFileOutput>,
    );
    async fn mark_file_error(
        &self,
        job_id: &JobId,
        filename: &str,
        error: &str,
        category: FailureCategory,
        finished_at: UnixTimestamp,
    );
    async fn start_file_attempt(
        &self,
        job_id: &JobId,
        filename: &str,
        work_unit_kind: WorkUnitKind,
        started_at: UnixTimestamp,
    );
    async fn finish_file_attempt(
        &self,
        job_id: &JobId,
        filename: &str,
        outcome: AttemptOutcome,
        failure_category: Option<FailureCategory>,
        disposition: RetryDisposition,
        finished_at: UnixTimestamp,
    );
    async fn mark_file_retry_pending(
        &self,
        job_id: &JobId,
        filename: &str,
        retry_at: UnixTimestamp,
        category: FailureCategory,
        message: &str,
        finished_at: UnixTimestamp,
    );
    async fn clear_file_retry_state(&self, job_id: &JobId, filename: &str);
    async fn set_file_progress(
        &self,
        job_id: &JobId,
        filename: &str,
        stage: FileStage,
        current: Option<i64>,
        total: Option<i64>,
    );
    async fn unfinished_files(&self, job_id: &JobId) -> Vec<DisplayPath>;
    async fn file_status_label(&self, job_id: &JobId, filename: &str) -> Option<String>;
    async fn bump_forced_terminal_errors(&self, count: usize);
    async fn fail_job(&self, job_id: &JobId, error: &str, failed_at: UnixTimestamp);
    async fn mark_job_running(&self, job_id: &JobId);
    async fn record_job_worker_count(&self, job_id: &JobId, worker_count: usize);
    async fn requeue_job_after_memory_gate(&self, job_id: &JobId, retry_at: UnixTimestamp);
    async fn bump_deferred_work_units(&self);
    async fn bump_memory_gate_aborts(&self);
    async fn finalize_job(
        &self,
        job_id: &JobId,
        final_status: JobStatus,
        completed_at: UnixTimestamp,
    );
}

/// Store-backed implementation of the runner event sink.
#[derive(Clone)]
pub(crate) struct StoreRunnerEventSink {
    store: Arc<JobStore>,
}

impl StoreRunnerEventSink {
    /// Wrap one concrete store as the current runner event sink.
    ///
    /// Named `wrap` rather than `new` because the return type is the trait
    /// object `Arc<dyn RunnerEventSink>`, not `Self` — clippy's
    /// `new_ret_no_self` rule prefers the non-`new` name for such factories.
    pub(crate) fn wrap(store: Arc<JobStore>) -> Arc<dyn RunnerEventSink> {
        Arc::new(Self { store })
    }
}

#[async_trait]
impl RunnerEventSink for StoreRunnerEventSink {
    async fn mark_file_processing(
        &self,
        job_id: &JobId,
        filename: &str,
        started_at: UnixTimestamp,
    ) {
        self.store
            .mark_file_processing(job_id, filename, started_at)
            .await;
    }

    async fn mark_file_done(
        &self,
        job_id: &JobId,
        filename: &str,
        finished_at: UnixTimestamp,
        result: Option<CompletedFileOutput>,
    ) {
        self.store
            .mark_file_done(job_id, filename, finished_at, result)
            .await;
    }

    async fn mark_file_error(
        &self,
        job_id: &JobId,
        filename: &str,
        error: &str,
        category: FailureCategory,
        finished_at: UnixTimestamp,
    ) {
        self.store
            .mark_file_error(
                job_id,
                filename,
                &FileFailureRecord {
                    message: error.to_string(),
                    category,
                    finished_at,
                },
            )
            .await;
    }

    async fn start_file_attempt(
        &self,
        job_id: &JobId,
        filename: &str,
        work_unit_kind: WorkUnitKind,
        started_at: UnixTimestamp,
    ) {
        self.store
            .start_file_attempt(job_id, filename, work_unit_kind, started_at)
            .await;
    }

    async fn finish_file_attempt(
        &self,
        job_id: &JobId,
        filename: &str,
        outcome: AttemptOutcome,
        failure_category: Option<FailureCategory>,
        disposition: RetryDisposition,
        finished_at: UnixTimestamp,
    ) {
        self.store
            .db_finish_attempt_for_file(
                job_id,
                AttemptFinishRecord {
                    filename,
                    outcome,
                    failure_category,
                    disposition,
                    finished_at,
                },
            )
            .await;
    }

    async fn mark_file_retry_pending(
        &self,
        job_id: &JobId,
        filename: &str,
        retry_at: UnixTimestamp,
        category: FailureCategory,
        message: &str,
        finished_at: UnixTimestamp,
    ) {
        self.store
            .mark_file_retry_pending(
                job_id,
                filename,
                &FileRetryRecord {
                    message: message.to_string(),
                    category,
                    finished_at,
                    retry_at,
                },
            )
            .await;
    }

    async fn clear_file_retry_state(&self, job_id: &JobId, filename: &str) {
        self.store.clear_file_retry_state(job_id, filename).await;
    }

    async fn set_file_progress(
        &self,
        job_id: &JobId,
        filename: &str,
        stage: FileStage,
        current: Option<i64>,
        total: Option<i64>,
    ) {
        self.store
            .set_file_progress(
                job_id,
                filename,
                &FileProgressRecord {
                    stage: stage.api_stage(),
                    current,
                    total,
                },
            )
            .await;
    }

    async fn unfinished_files(&self, job_id: &JobId) -> Vec<DisplayPath> {
        self.store.unfinished_files(job_id).await
    }

    async fn file_status_label(&self, job_id: &JobId, filename: &str) -> Option<String> {
        self.store.file_status_label(job_id, filename).await
    }

    async fn bump_forced_terminal_errors(&self, count: usize) {
        self.store
            .bump_counter(|c| c.forced_terminal_errors += count as i64)
            .await;
    }

    async fn fail_job(&self, job_id: &JobId, error: &str, failed_at: UnixTimestamp) {
        self.store.fail_job(job_id, error, failed_at).await;
    }

    async fn mark_job_running(&self, job_id: &JobId) {
        self.store.mark_job_running(job_id).await;
    }

    async fn record_job_worker_count(&self, job_id: &JobId, worker_count: usize) {
        self.store
            .record_job_worker_count(job_id, worker_count)
            .await;
    }

    async fn requeue_job_after_memory_gate(&self, job_id: &JobId, retry_at: UnixTimestamp) {
        self.store
            .requeue_job_after_memory_gate(job_id, retry_at)
            .await;
    }

    async fn bump_deferred_work_units(&self) {
        self.store
            .bump_counter(|c| c.deferred_work_units += 1)
            .await;
    }

    async fn bump_memory_gate_aborts(&self) {
        self.store.bump_counter(|c| c.memory_gate_aborts += 1).await;
    }

    async fn finalize_job(
        &self,
        job_id: &JobId,
        final_status: JobStatus,
        completed_at: UnixTimestamp,
    ) {
        self.store
            .finalize_job(job_id, final_status, completed_at)
            .await;
    }
}
