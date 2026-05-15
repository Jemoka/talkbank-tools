//! Queue lease management for job dispatch.
//!
//! Lease methods control whether a job can be claimed by the local queue
//! dispatcher and manage the heartbeat/expiry lifecycle of active leases.
//! The local queue-claim path is currently test-only but the lease primitives
//! (`clear_lease`, `release_local_dispatch_claim`, `renew_local_dispatch_lease`)
//! are used by the production server during runner teardown and heartbeat
//! renewal.

use crate::api::{NodeId, UnixTimestamp};
use crate::scheduling::LeaseRecord;

use super::Job;

impl Job {
    /// Clear the current queue lease metadata.
    pub(crate) fn clear_lease(&mut self) {
        self.schedule.lease.leased_by_node = None;
        self.schedule.lease.expires_at = None;
        self.schedule.lease.heartbeat_at = None;
    }

    /// Return whether a live queue lease currently blocks local dispatch.
    ///
    /// Currently exercised only by the test-only local queue-claim path.
    #[cfg(test)]
    pub(crate) fn lease_blocks_local_dispatch(&self, now: UnixTimestamp) -> bool {
        self.schedule.lease.leased_by_node.is_some()
            && self
                .schedule
                .lease
                .expires_at
                .is_some_and(|timestamp| timestamp.0 > now.0)
    }

    /// Return the earliest time when the job should be reconsidered for dispatch.
    ///
    /// Currently exercised only by the test-only local queue-claim path.
    #[cfg(test)]
    pub(crate) fn next_local_dispatch_wake_at(&self, now: UnixTimestamp) -> Option<UnixTimestamp> {
        let mut wake_at = self
            .schedule
            .next_eligible_at
            .filter(|timestamp| timestamp.0 > now.0);
        if self.lease_blocks_local_dispatch(now) {
            wake_at = match (wake_at, self.schedule.lease.expires_at) {
                (Some(next_eligible_at), Some(lease_expires_at)) => {
                    if next_eligible_at.0 < lease_expires_at.0 {
                        Some(next_eligible_at)
                    } else {
                        Some(lease_expires_at)
                    }
                }
                (None, Some(lease_expires_at)) => Some(lease_expires_at),
                (some, None) => some,
            };
        }
        wake_at
    }

    /// Return whether the job can be claimed by the local queue dispatcher now.
    ///
    /// Currently exercised only by the test-only local queue-claim path.
    #[cfg(test)]
    pub(crate) fn ready_for_local_dispatch(&self, now: UnixTimestamp) -> bool {
        self.execution.status == crate::api::JobStatus::Queued
            && !self.runtime.runner_active
            && !self.lease_blocks_local_dispatch(now)
            && self
                .schedule
                .next_eligible_at
                .is_none_or(|timestamp| timestamp <= now)
    }

    /// Claim the job for local dispatch and return the resulting lease record.
    ///
    /// Currently exercised only by the test-only local queue-claim path.
    #[cfg(test)]
    pub(crate) fn claim_for_local_dispatch(
        &mut self,
        node_id: &NodeId,
        now: UnixTimestamp,
        lease_ttl_s: f64,
    ) -> Option<LeaseRecord> {
        if !self.ready_for_local_dispatch(now) {
            return None;
        }

        self.runtime.runner_active = true;
        self.schedule.lease.leased_by_node = Some(node_id.clone());
        self.schedule.lease.heartbeat_at = Some(now);
        self.schedule.lease.expires_at = Some(UnixTimestamp(now.0 + lease_ttl_s));
        self.active_lease()
    }

    /// Release any local dispatch claim and clear the job's live lease.
    pub(crate) fn release_local_dispatch_claim(&mut self) {
        self.runtime.runner_active = false;
        self.clear_lease();
    }

    /// Renew the local dispatch lease when the current node still owns it.
    pub(crate) fn renew_local_dispatch_lease(
        &mut self,
        node_id: &NodeId,
        now: UnixTimestamp,
        lease_ttl_s: f64,
    ) -> Option<LeaseRecord> {
        if self.runtime.runner_active
            && self.schedule.lease.leased_by_node.as_deref() == Some(node_id)
            && !self.execution.status.is_terminal()
        {
            self.schedule.lease.heartbeat_at = Some(now);
            self.schedule.lease.expires_at = Some(UnixTimestamp(now.0 + lease_ttl_s));
            self.active_lease()
        } else {
            None
        }
    }

    /// Build a `LeaseRecord` from the job's current lease fields, if all are set.
    pub(crate) fn active_lease(&self) -> Option<LeaseRecord> {
        Some(LeaseRecord {
            leased_by_node: self.schedule.lease.leased_by_node.clone()?,
            heartbeat_at: self.schedule.lease.heartbeat_at?,
            expires_at: self.schedule.lease.expires_at?,
        })
    }
}
