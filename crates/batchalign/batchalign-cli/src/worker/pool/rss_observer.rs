//! Per-process RSS observation for worker-pool admission (Mode B).
//!
//! # Why this exists
//!
//! Mode A's memory gate (`memory_gate.rs`) uses each profile's
//! tier-derived `startup_reservation_mb` as the projection of "how
//! much will the new worker consume?" That estimate is conservative
//! by design: it covers the *startup peak* during model load, not
//! the steady-state RSS after the worker settles into idle. On
//! long-running daemons whose workers have settled, reservation
//! over-estimates by 2–3× (e.g., a Stanza worker's 12 GB reservation
//! vs ~4 GB steady RSS).
//!
//! Mode B replaces the static reservation with a *measured* estimate
//! drawn from same-profile idle workers' actual RSS. When idle peers
//! exist, we know what a warmed-up worker of this profile actually
//! consumes; that's a tighter projection and admits more workers
//! when the host has the headroom.
//!
//! # Why idle workers, not all live workers
//!
//! `WorkerGroup.idle` holds workers that have completed model load
//! and returned to the pool — settled steady-state. Checked-out
//! workers are mid-inference and may carry transient request data
//! that overstates steady-state RSS. Workers loading their model
//! aren't in `idle` yet (they're held inside `try_spawn_into_group`
//! until ready). Idle is the cleanest sample for "what does a
//! warmed-up worker of this profile cost?"
//!
//! # Bootstrap
//!
//! When no idle peers exist (first spawn of the profile after pool
//! startup, or all peers currently checked out), the caller falls
//! back to the static reservation — same as Mode A. This means Mode
//! B is strictly an *opt-in* improvement over Mode A: when peer
//! observation is unavailable, behavior matches Mode A exactly.
//!
//! # Sysinfo cost
//!
//! Each admission attempt opens one `sysinfo::System` and refreshes
//! the specific worker PIDs we're interested in. Order ~ms for ≤10
//! PIDs. The cost is in the cold admission path; if `try_claim_spawn_slot`
//! ever moves to the hot per-request path, this should shift to
//! background polling with a cached read.

use std::collections::HashMap;
use std::sync::Arc;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::worker::{WorkerPid, WorkerProfile};

use super::{GroupsMap, WorkerGroup, lock_recovered};

/// Origin of the per-spawn memory estimate, surfaced via
/// `MemoryConstrained` for diagnostic logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EstimateSource {
    /// No same-profile idle peers were available. Fell back to the
    /// static `startup_reservation_mb_for_tier` value — Mode A
    /// behavior.
    Reservation,
    /// Same-profile idle peers were sampled; the average of their
    /// observed RSS is the estimate. Mode B behavior.
    ObservedAvgIdle,
}

/// Sample observed average RSS in MB across idle workers of the
/// given profile. Returns `None` when no idle peers exist (caller
/// must fall back to reservation).
///
/// The walk holds each `WorkerGroup.idle` mutex briefly to snapshot
/// PIDs, then releases it before the sysinfo refresh — sysinfo's
/// `refresh_processes_specifics` can be tens of ms in the worst
/// case, and holding the idle mutex across that would block
/// `checkout()` and `push_back()` callers unnecessarily.
pub(super) fn observed_avg_rss_mb_for_profile(
    groups: &GroupsMap,
    profile: WorkerProfile,
) -> Option<u64> {
    let pids = collect_idle_pids_for_profile(groups, profile);
    if pids.is_empty() {
        return None;
    }
    sample_avg_rss_mb_for_pids(&pids)
}

/// Walk `groups`, collect PIDs of idle workers in groups whose
/// profile matches. Each `idle` mutex is held only long enough to
/// `iter()` the queue and copy out PIDs.
fn collect_idle_pids_for_profile(groups: &GroupsMap, profile: WorkerProfile) -> Vec<WorkerPid> {
    let group_arcs: Vec<Arc<WorkerGroup>> = {
        let groups = lock_recovered(groups);
        groups
            .values()
            .filter(|g| g.profile == profile)
            .cloned()
            .collect()
    };
    let mut pids = Vec::new();
    for group in &group_arcs {
        let idle = lock_recovered(&group.idle);
        for handle in idle.iter() {
            pids.push(handle.pid());
        }
    }
    pids
}

/// Refresh sysinfo for the given PIDs and return per-PID RSS in MB
/// for the ones that are still alive. PIDs that have died (or were
/// never live) are silently absent from the result map; callers
/// distinguish "missing" from "live with zero RSS" by membership.
///
/// Shared by `observed_avg_rss_mb_for_profile` (averages across the
/// values) and `idle_eviction::snapshot_idle_workers_with_rss`
/// (joins with caller-provided metadata). Both sites previously
/// hand-rolled the same `Pid::from_u32 + System::new + refresh +
/// per-PID process lookup` triple; consolidating here keeps the
/// sysinfo refresh shape (use_kind = `with_memory()`,
/// remove_dead = `true`) in one place.
pub(super) fn refresh_rss_mb_for_pids(pids: &[WorkerPid]) -> HashMap<WorkerPid, u64> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let sysinfo_pids: Vec<Pid> = pids.iter().map(|p| Pid::from_u32(p.0)).collect();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&sysinfo_pids),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    let mut out = HashMap::with_capacity(pids.len());
    for pid in pids {
        let sysinfo_pid = Pid::from_u32(pid.0);
        if let Some(process) = sys.process(sysinfo_pid) {
            out.insert(*pid, process.memory() / (1024 * 1024));
        }
    }
    out
}

/// Average RSS (MB) across live PIDs, or `None` when none are live.
fn sample_avg_rss_mb_for_pids(pids: &[WorkerPid]) -> Option<u64> {
    let rss_by_pid = refresh_rss_mb_for_pids(pids);
    if rss_by_pid.is_empty() {
        return None;
    }
    let sum_mb: u128 = rss_by_pid.values().map(|v| *v as u128).sum();
    let avg_mb = sum_mb / rss_by_pid.len() as u128;
    Some(avg_mb as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sampling an empty PID list returns `None` so the caller falls
    /// back to reservation. This pins the invariant that callers can
    /// trust `None` to mean "no peers — use reservation."
    #[test]
    fn empty_pid_list_returns_none() {
        let result = sample_avg_rss_mb_for_pids(&[]);
        assert!(
            result.is_none(),
            "empty PIDs must return None, got {result:?}"
        );
    }

    /// Sampling our own PID returns a positive RSS value. Pins that
    /// the sysinfo wiring works end-to-end on this platform.
    #[test]
    fn sampling_self_pid_returns_positive_rss() {
        let our_pid = WorkerPid(std::process::id());
        let result = sample_avg_rss_mb_for_pids(&[our_pid]);
        let Some(rss_mb) = result else {
            panic!("expected Some(rss_mb) for self pid, got None");
        };
        assert!(rss_mb > 0, "self RSS must be positive, got {rss_mb} MB");
    }

    /// Sampling a list of all-dead PIDs returns `None`. Uses
    /// `u32::MAX` which cannot be a real PID on any platform.
    #[test]
    fn sampling_all_dead_pids_returns_none() {
        let result = sample_avg_rss_mb_for_pids(&[WorkerPid(u32::MAX)]);
        assert!(
            result.is_none(),
            "all-dead PIDs must return None, got {result:?}"
        );
    }

    /// `refresh_rss_mb_for_pids` is the shared primitive both
    /// `rss_observer` and `idle_eviction` use; the helper must
    /// preserve the (PID → live-RSS) mapping so callers can attach
    /// metadata to the result. Self-PID is always live.
    #[test]
    fn refresh_returns_self_pid_keyed_to_positive_rss() {
        let our_pid = WorkerPid(std::process::id());
        let map = refresh_rss_mb_for_pids(&[our_pid]);
        let rss_mb = map
            .get(&our_pid)
            .copied()
            .expect("self PID must be present in refresh result");
        assert!(rss_mb > 0, "self RSS must be positive, got {rss_mb} MB");
    }
}
