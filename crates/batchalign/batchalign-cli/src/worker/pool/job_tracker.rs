//! Per-job tracking of checked-out worker PIDs for cancel-driven shutdown.
//!
//! Without this, cancel waits for the in-flight worker call (Whisper,
//! Stanza, etc.) to complete naturally — minutes for long ASR passes.
//! `WorkerPool::shutdown_workers_for_job` drains the relevant PIDs
//! and SIGTERMs them so the dispatch future returns BrokenPipe and
//! the runner unwinds at the next iteration.
//!
//! Registration is opt-in: callers under a `CURRENT_JOB_ID` scope
//! (set by the runner) get tracked; warmup / health / discovery
//! paths run without the scope and skip registration.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::api::JobId;
use crate::worker::WorkerPid;
use dashmap::DashMap;
use tracing::{debug, info};

tokio::task_local! {
    /// Set at the entry of `run_server_job_attempt` so dispatch-site
    /// `TrackerGuard`s register against the correct job. The
    /// task-local propagates across awaits transparently — no
    /// signature changes at intermediate dispatch sites.
    pub(crate) static CURRENT_JOB_ID: JobId;
}

pub(crate) fn current_job_id() -> Option<JobId> {
    CURRENT_JOB_ID.try_with(|j| j.clone()).ok()
}

/// Thread-safe map from job to checked-out worker PIDs. Uses
/// `DashMap` for sharded per-key concurrency — the dispatch hot
/// path can register/unregister without contending on a single
/// global Mutex. The per-job `HashSet` stays under a small mutex
/// because DashMap doesn't expose a per-entry write API for nested
/// containers; contention there is per-job, not pool-wide.
#[derive(Debug, Clone, Default)]
pub(crate) struct JobWorkerTracker {
    by_job: Arc<DashMap<JobId, Mutex<HashSet<WorkerPid>>>>,
}

impl JobWorkerTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(&self, job_id: &JobId, pid: WorkerPid) {
        let entry = self.by_job.entry(job_id.clone()).or_default();
        entry
            .value()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(pid);
    }

    pub(crate) fn unregister(&self, job_id: &JobId, pid: WorkerPid) {
        let now_empty = {
            let Some(entry) = self.by_job.get(job_id) else {
                return;
            };
            let mut set = entry.value().lock().unwrap_or_else(|e| e.into_inner());
            set.remove(&pid);
            set.is_empty()
        };
        if now_empty {
            self.by_job.remove_if(job_id, |_, mu| {
                mu.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
            });
        }
    }

    /// Take and return all PIDs for `job_id`, clearing the entry.
    /// Caller signals outside any lock so syscalls don't serialize
    /// per-job updates from other tasks.
    pub(crate) fn drain(&self, job_id: &JobId) -> Vec<WorkerPid> {
        match self.by_job.remove(job_id) {
            Some((_, mu)) => mu
                .into_inner()
                .unwrap_or_else(|e| e.into_inner())
                .into_iter()
                .collect(),
            None => Vec::new(),
        }
    }

    /// PIDs registered for `job_id`, without draining. Public via
    /// `WorkerPool::workers_for_job` for integration tests that need
    /// to wait for dispatch registration before firing the cancel.
    pub(crate) fn snapshot(&self, job_id: &JobId) -> Vec<WorkerPid> {
        self.by_job
            .get(job_id)
            .map(|entry| {
                entry
                    .value()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// RAII registration of `(job, pid)` for the duration of a worker
/// dispatch. Unregisters on drop — including on panic unwind, so
/// the side-table never accumulates ghosts. Constructing with no
/// active `CURRENT_JOB_ID` scope is a no-op.
#[must_use = "guard must be held for the dispatch lifetime"]
pub(crate) struct TrackerGuard {
    tracker: JobWorkerTracker,
    job_id: Option<JobId>,
    pid: WorkerPid,
}

impl TrackerGuard {
    pub(crate) fn new(tracker: &JobWorkerTracker, pid: WorkerPid) -> Self {
        let job_id = current_job_id();
        if let Some(ref jid) = job_id {
            tracker.register(jid, pid);
        }
        Self {
            tracker: tracker.clone(),
            job_id,
            pid,
        }
    }
}

impl Drop for TrackerGuard {
    fn drop(&mut self) {
        if let Some(ref jid) = self.job_id {
            self.tracker.unregister(jid, self.pid);
        }
    }
}

/// Send SIGTERM to every PID, wait `grace`, SIGKILL any survivors.
/// Async-safe analog to `pool::reaper::kill_orphan`; both share the
/// `terminate_pgid` / `kill_pgid` / `process_alive` primitives in
/// `pool/reaper.rs`.
pub(crate) async fn signal_workers(pids: &[WorkerPid], grace: Duration) {
    if pids.is_empty() {
        return;
    }
    for pid in pids {
        super::reaper::terminate_pgid(pid.0);
    }
    tokio::time::sleep(grace).await;
    for pid in pids {
        if super::reaper::process_alive(pid.0) {
            info!(
                worker_pid = pid.0,
                "Worker survived SIGTERM, sending SIGKILL"
            );
            super::reaper::kill_pgid(pid.0);
        } else {
            debug!(worker_pid = pid.0, "Worker exited after SIGTERM");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u32) -> WorkerPid {
        WorkerPid(n)
    }
    fn job(s: &str) -> JobId {
        JobId::from(s.to_string())
    }

    fn total(t: &JobWorkerTracker) -> usize {
        t.by_job
            .iter()
            .map(|entry| {
                entry
                    .value()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .len()
            })
            .sum()
    }

    #[test]
    fn register_and_drain_returns_all_pids() {
        let t = JobWorkerTracker::new();
        t.register(&job("a"), pid(1));
        t.register(&job("a"), pid(2));
        t.register(&job("b"), pid(3));

        let mut drained_a = t.drain(&job("a"));
        drained_a.sort_by_key(|p| p.0);
        assert_eq!(drained_a, vec![pid(1), pid(2)]);

        assert!(t.snapshot(&job("a")).is_empty());
        assert_eq!(t.snapshot(&job("b")), vec![pid(3)]);
        assert_eq!(total(&t), 1);
    }

    #[test]
    fn unregister_removes_one_pid_leaves_others() {
        let t = JobWorkerTracker::new();
        t.register(&job("a"), pid(1));
        t.register(&job("a"), pid(2));
        t.unregister(&job("a"), pid(1));
        assert_eq!(t.snapshot(&job("a")), vec![pid(2)]);
        assert_eq!(total(&t), 1);
    }

    #[test]
    fn unregister_last_pid_drops_empty_entry() {
        let t = JobWorkerTracker::new();
        t.register(&job("a"), pid(1));
        t.unregister(&job("a"), pid(1));
        assert!(t.snapshot(&job("a")).is_empty());
        assert_eq!(total(&t), 0);
    }

    #[test]
    fn unregister_unknown_pid_is_noop() {
        let t = JobWorkerTracker::new();
        t.unregister(&job("ghost"), pid(99));
        assert_eq!(total(&t), 0);
    }

    #[test]
    fn drain_unknown_job_returns_empty() {
        let t = JobWorkerTracker::new();
        assert!(t.drain(&job("ghost")).is_empty());
    }

    #[test]
    fn register_same_pid_twice_is_idempotent() {
        let t = JobWorkerTracker::new();
        t.register(&job("a"), pid(1));
        t.register(&job("a"), pid(1));
        assert_eq!(t.snapshot(&job("a")), vec![pid(1)]);
    }

    #[test]
    fn clone_shares_state() {
        let t1 = JobWorkerTracker::new();
        let t2 = t1.clone();
        t1.register(&job("a"), pid(1));
        assert_eq!(t2.snapshot(&job("a")), vec![pid(1)]);
    }

    #[tokio::test]
    async fn signal_workers_empty_is_noop() {
        signal_workers(&[], Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn signal_workers_unknown_pid_does_not_panic() {
        signal_workers(&[pid(999_999_999)], Duration::from_millis(10)).await;
    }

    /// Spawn a real `sleep` subprocess, fire `signal_workers`,
    /// assert the process is gone within the grace window. Closest
    /// portable analog to the PID 15650 zombie scenario without
    /// the test-echo Python worker harness.
    #[cfg(unix)]
    #[tokio::test]
    async fn signal_workers_terminates_real_subprocess() {
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep");
        let real_pid = WorkerPid(child.id());
        assert!(super::super::reaper::process_alive(real_pid.0));

        signal_workers(&[real_pid], Duration::from_secs(2)).await;

        let _ = child.wait();
        assert!(!super::super::reaper::process_alive(real_pid.0));
    }
}
