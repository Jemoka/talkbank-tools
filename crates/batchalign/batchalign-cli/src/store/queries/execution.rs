//! Runner-owned job execution mutations on [`JobStore`].

use crate::api::{JobId, JobStatus, UnixTimestamp};

use super::super::registry::JobCompletionSnapshot;
use super::super::{JobStore, PersistedJobUpdate};

impl JobStore {
    /// Re-queue a job after memory-gate rejection and persist the retry deadline.
    pub(crate) async fn requeue_job_after_memory_gate(
        &self,
        job_id: &JobId,
        retry_at: UnixTimestamp,
    ) {
        let Some(job_update) = self
            .registry
            .requeue_after_memory_gate(job_id, retry_at)
            .await
        else {
            return;
        };
        self.notify_job_item(job_update);

        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: JobStatus::Queued,
                error: None,
                completed_at: None,
                num_workers: None,
                next_eligible_at: Some(retry_at),
            },
        )
        .await;
    }

    /// Mark a job as running and clear any deferred retry deadline.
    pub(crate) async fn mark_job_running(&self, job_id: &JobId) {
        let Some(job_update) = self.registry.mark_job_running(job_id).await else {
            return;
        };
        self.notify_job_item(job_update);

        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: JobStatus::Running,
                error: None,
                completed_at: None,
                num_workers: None,
                next_eligible_at: None,
            },
        )
        .await;
    }

    /// Record the per-job worker count chosen for this run.
    pub(crate) async fn record_job_worker_count(&self, job_id: &JobId, num_workers: usize) {
        if !self
            .registry
            .record_job_worker_count(job_id, num_workers)
            .await
        {
            return;
        }

        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: JobStatus::Running,
                error: None,
                completed_at: None,
                num_workers: Some(num_workers as i32),
                next_eligible_at: None,
            },
        )
        .await;
    }

    /// Fail a job immediately with a job-level error message.
    pub(crate) async fn fail_job(&self, job_id: &JobId, error: &str, completed_at: UnixTimestamp) {
        let Some(job_update) = self.registry.fail_job(job_id, error, completed_at).await else {
            return;
        };
        self.notify_job_item(job_update);

        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: JobStatus::Failed,
                error: Some(error),
                completed_at: Some(completed_at),
                num_workers: None,
                next_eligible_at: None,
            },
        )
        .await;
    }

    /// Return the cancellation and aggregate file-outcome facts for one job.
    pub(crate) async fn completion_snapshot(
        &self,
        job_id: &JobId,
    ) -> Option<JobCompletionSnapshot> {
        self.registry.completion_snapshot(job_id).await
    }

    /// Finalize a job after all file tasks have stopped mutating its state.
    pub(crate) async fn finalize_job(
        &self,
        job_id: &JobId,
        final_status: JobStatus,
        completed_at: UnixTimestamp,
    ) {
        let Some(job_update) = self
            .registry
            .finalize_job(job_id, final_status, completed_at)
            .await
        else {
            return;
        };
        self.notify_job_item(job_update);

        self.db_update_job(
            job_id,
            PersistedJobUpdate {
                status: final_status,
                error: None,
                completed_at: Some(completed_at),
                num_workers: None,
                next_eligible_at: None,
            },
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    use crate::api::{FileStatusKind, JobId, JobStatus, ReleasedCommand, UnixTimestamp};
    use crate::store::queries::tests::{make_job, test_config};
    use crate::ws::BROADCAST_CAPACITY;

    use super::*;

    /// Re-queued jobs retain queued status and gain the next retry timestamp.
    #[tokio::test]
    async fn requeue_job_after_memory_gate_sets_retry_deadline() {
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

        let retry_at = UnixTimestamp(321.0);
        store
            .requeue_job_after_memory_gate(&JobId::from("job-1"), retry_at)
            .await;

        let info = store.get(&JobId::from("job-1")).await.unwrap();
        assert_eq!(info.status, JobStatus::Queued);
        assert_eq!(info.next_eligible_at, Some(retry_at));
    }

    /// Finalization recomputes completed file counts from terminal file state.
    #[tokio::test]
    async fn finalize_job_recounts_terminal_files() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);
        let mut job = make_job(
            "job-1",
            ReleasedCommand::Morphotag,
            vec!["a.cha".into(), "b.cha".into()],
        );
        job.execution.file_statuses.get_mut("a.cha").unwrap().status = FileStatusKind::Done;
        store.submit(job).await.unwrap();

        store
            .finalize_job(
                &JobId::from("job-1"),
                JobStatus::Completed,
                UnixTimestamp(500.0),
            )
            .await;

        let info = store.get(&JobId::from("job-1")).await.unwrap();
        assert_eq!(info.status, JobStatus::Completed);
        assert_eq!(info.completed_files, 1);
    }
}
