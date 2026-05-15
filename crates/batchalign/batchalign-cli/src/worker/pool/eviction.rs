//! Idle-worker eviction for `WorkerPool::checkout` saturation handling.
//!
//! When the pool's global cap is saturated and the requesting key has zero
//! live workers, pop an idle worker from another group to free a slot for
//! the new key. Without this, a saturated pool that happens to hold idle
//! workers of unrelated keys rejects new work with a "would deadlock"
//! error even though a slot could be freed synchronously.
//!
//! [`select_eviction_target`] is the pure selection rule (tested in
//! isolation); [`WorkerPool::try_evict_idle_from_other_group`] is the
//! imperative wrapper that acquires the live state.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::Ordering;

use tracing::info;

use super::{WorkerKey, WorkerPool, lock_recovered};

/// Immutable per-group view used by the pure eviction-target selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GroupSnapshot {
    pub idle_count: usize,
}

/// Pick an eviction target: any group other than `skip_key` with at least
/// one idle worker. Ties are broken by picking the group with the highest
/// idle count so the most "wasted" slot is freed first.
pub(super) fn select_eviction_target<K>(
    groups: &HashMap<K, GroupSnapshot>,
    skip_key: &K,
) -> Option<K>
where
    K: Clone + Eq + Hash,
{
    groups
        .iter()
        .filter(|(key, snap)| *key != skip_key && snap.idle_count > 0)
        .max_by_key(|(_, snap)| snap.idle_count)
        .map(|(key, _)| key.clone())
}

/// Result of a single eviction attempt. Named so callers don't have to
/// read `bool` and guess which way is "we freed a slot".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EvictionOutcome {
    /// A slot was freed; the caller should retry spawn.
    Evicted,
    /// No other group had an idle worker (or a race stole the one we saw).
    NoIdleElsewhere,
}

impl WorkerPool {
    /// Evict one idle worker from any group other than `skip_key`.
    ///
    /// Snapshots idle counts under the groups-map lock to avoid nested
    /// idle-then-groups lock acquisitions elsewhere (the established
    /// order in this crate is groups → idle). Runs the evicted handle's
    /// drop on a detached task so the on-request-path `checkout` never
    /// waits on worker shutdown.
    pub(super) fn try_evict_idle_from_other_group(&self, skip_key: &WorkerKey) -> EvictionOutcome {
        let victim_group = {
            let groups = lock_recovered(&self.groups);
            let snapshots: HashMap<WorkerKey, GroupSnapshot> = groups
                .iter()
                .map(|(key, group)| {
                    let idle_count = lock_recovered(&group.idle).len();
                    (key.clone(), GroupSnapshot { idle_count })
                })
                .collect();
            let Some(victim_key) = select_eviction_target(&snapshots, skip_key) else {
                return EvictionOutcome::NoIdleElsewhere;
            };
            match groups.get(&victim_key) {
                Some(group) => (victim_key, group.clone()),
                None => return EvictionOutcome::NoIdleElsewhere,
            }
        };
        let (victim_key, victim_group) = victim_group;

        let Ok(permit) = victim_group.available.try_acquire() else {
            return EvictionOutcome::NoIdleElsewhere;
        };
        permit.forget();

        let Some(handle) = lock_recovered(&victim_group.idle).pop_front() else {
            // Permit without a matching idle worker — restore the
            // invariant (permits == idle.len()) and bail.
            victim_group.available.add_permits(1);
            return EvictionOutcome::NoIdleElsewhere;
        };

        victim_group.total.fetch_sub(1, Ordering::Relaxed);
        // The victim worker is gone; refund its global-cap permit so
        // the requester (or any other admission) can acquire it on
        // the next try_claim_spawn_slot loop iteration.
        victim_group.spawn_permits.add_permits(1);
        info!(
            victim = ?victim_key,
            requesting = ?skip_key,
            "Evicting idle worker to free a global-cap slot"
        );
        // Detach the handle drop (WorkerHandle::Drop sends SIGTERM+SIGKILL).
        // At runtime-shutdown the reaper in lifecycle.rs cleans up any
        // process this task would have killed.
        tokio::spawn(async move {
            drop(handle);
        });

        EvictionOutcome::Evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(idle: usize) -> GroupSnapshot {
        GroupSnapshot { idle_count: idle }
    }

    #[test]
    fn select_returns_none_when_only_skip_key_has_idle() {
        let mut groups: HashMap<&'static str, GroupSnapshot> = HashMap::new();
        groups.insert("deu", snap(2));
        groups.insert("eng", snap(0));
        assert_eq!(select_eviction_target(&groups, &"deu"), None);
    }

    #[test]
    fn select_returns_none_when_no_group_has_idle_workers() {
        let mut groups: HashMap<&'static str, GroupSnapshot> = HashMap::new();
        groups.insert("eng", snap(0));
        groups.insert("deu", snap(0));
        assert_eq!(select_eviction_target(&groups, &"deu"), None);
    }

    #[test]
    fn select_picks_other_group_with_idle_workers() {
        let mut groups: HashMap<&'static str, GroupSnapshot> = HashMap::new();
        groups.insert("eng", snap(3));
        groups.insert("deu", snap(0));
        assert_eq!(select_eviction_target(&groups, &"deu"), Some("eng"));
    }

    /// Regression guard for the saturation pattern: multiple idle groups
    /// exist, picker prefers the one with the highest idle count so the
    /// most wasted slot frees first.
    #[test]
    fn select_prefers_group_with_highest_idle_count() {
        let mut groups: HashMap<&'static str, GroupSnapshot> = HashMap::new();
        groups.insert("eng", snap(4));
        groups.insert("fra", snap(1));
        groups.insert("deu", snap(0));
        assert_eq!(select_eviction_target(&groups, &"deu"), Some("eng"));
    }

    #[test]
    fn select_never_returns_the_skip_key_even_if_its_idle_count_is_highest() {
        let mut groups: HashMap<&'static str, GroupSnapshot> = HashMap::new();
        groups.insert("deu", snap(9));
        groups.insert("eng", snap(1));
        assert_eq!(select_eviction_target(&groups, &"deu"), Some("eng"));
    }
}
