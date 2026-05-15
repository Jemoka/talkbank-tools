//! Runner-facing projections over the job store.

use crate::api::JobId;

use super::super::JobStore;
use super::super::job::RunnerJobSnapshot;

impl JobStore {
    /// Return the immutable runner-facing snapshot for one job.
    pub async fn runner_snapshot(&self, job_id: &JobId) -> Option<RunnerJobSnapshot> {
        self.registry.runner_snapshot(job_id).await
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    use crate::ws::BROADCAST_CAPACITY;

    use super::super::tests::{make_job, test_config};
    use super::*;
    use crate::api::ReleasedCommand;

    /// Runner snapshots include only unfinished files and clone stable config.
    #[tokio::test]
    async fn runner_snapshot_filters_terminal_files() {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = JobStore::new(test_config(), None, tx);

        let mut job = make_job(
            "j1",
            ReleasedCommand::Morphotag,
            vec!["a.cha".into(), "b.cha".into()],
        );
        if let Some(status) = job.execution.file_statuses.get_mut("a.cha") {
            status.status = crate::api::FileStatusKind::Done;
        }
        store.submit(job).await.unwrap();

        let snapshot = store
            .runner_snapshot(&JobId::from("j1"))
            .await
            .expect("runner snapshot");

        assert_eq!(snapshot.dispatch.command.as_ref(), "morphotag");
        assert_eq!(snapshot.pending_files.len(), 1);
        assert_eq!(snapshot.pending_files[0].filename.as_ref(), "b.cha");
    }
}
