//! Queue dispatch and lease management.

use crate::api::JobId;
#[cfg(test)]
use crate::queue::QueuePoll;
use tracing::warn;

use super::super::{JobStore, unix_now};

/// Result of attempting to renew the local queue lease heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaseRenewalOutcome {
    /// The lease is still active and was renewed.
    Renewed,
    /// The lease should no longer be renewed.
    Stop,
}

impl JobStore {
    /// Return IDs of jobs that are in QUEUED status (for auto-resume on startup).
    pub(crate) async fn queued_job_ids(&self) -> Vec<JobId> {
        self.registry.queued_job_ids().await
    }

    /// Claim queued jobs that are eligible to run now.
    ///
    /// Currently exercised only by the local queue-claim tests.
    #[cfg(test)]
    pub(crate) async fn claim_ready_queued_jobs(&self) -> QueuePoll {
        let now = unix_now();
        let claimed = self
            .registry
            .claim_ready_queued_jobs(now, self.node_id(), self.local_lease_ttl_s())
            .await;

        for claimed in &claimed.claimed_leases {
            if let Some(db) = &self.db
                && let Err(e) = db
                    .update_job_lease(
                        &claimed.job_id,
                        Some(claimed.lease.leased_by_node.as_ref()),
                        Some(claimed.lease.expires_at.0),
                        Some(claimed.lease.heartbeat_at.0),
                    )
                    .await
            {
                warn!(job_id = %claimed.job_id, error = %e, "DB update_job_lease failed on claim");
            }
        }

        claimed.poll
    }

    /// Release the runner claim so a queued job may be re-dispatched later.
    pub(crate) async fn release_runner_claim(&self, job_id: &JobId) {
        if !self.registry.release_runner_claim(job_id).await {
            return;
        }

        if let Some(db) = &self.db
            && let Err(e) = db.update_job_lease(job_id, None, None, None).await
        {
            warn!(job_id = %job_id, error = %e, "DB update_job_lease failed on release");
        }
    }

    /// Renew the local lease for a currently claimed job.
    ///
    /// Returns [`LeaseRenewalOutcome::Renewed`] while the local claim is still
    /// active, or [`LeaseRenewalOutcome::Stop`] when the heartbeat loop should exit.
    pub(crate) async fn renew_job_lease(&self, job_id: &JobId) -> LeaseRenewalOutcome {
        let now = unix_now();
        let renewed_lease = self
            .registry
            .renew_job_lease(job_id, self.node_id(), now, self.local_lease_ttl_s())
            .await;

        if let Some(lease) = renewed_lease.clone()
            && let Some(db) = &self.db
            && let Err(e) = db
                .update_job_lease(
                    job_id,
                    Some(lease.leased_by_node.as_ref()),
                    Some(lease.expires_at.0),
                    Some(lease.heartbeat_at.0),
                )
                .await
        {
            warn!(job_id = %job_id, error = %e, "DB update_job_lease failed on renew");
        }

        if renewed_lease.is_some() {
            LeaseRenewalOutcome::Renewed
        } else {
            LeaseRenewalOutcome::Stop
        }
    }
}
