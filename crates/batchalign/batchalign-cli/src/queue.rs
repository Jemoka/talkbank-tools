//! Queue dispatch types shared by the store and backend modules.
//!
//! Currently used only by test code that exercises the local queue-claim path.

/// Result of polling a queue backend for currently eligible queued jobs.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct QueuePoll {
    /// Job IDs that are ready to run now and have been claimed by the backend.
    pub ready_job_ids: Vec<crate::api::JobId>,
    /// Earliest future eligibility timestamp among still-queued jobs.
    pub next_wake_at: Option<crate::api::UnixTimestamp>,
}
