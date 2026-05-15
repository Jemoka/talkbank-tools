//! Global-cap admission permits.
//!
//! [`SpawnPermitGuard`] is an RAII wrapper around
//! [`tokio::sync::OwnedSemaphorePermit`] used to enforce
//! `max_total_workers` across the whole pool. The lifetime semantics
//! follow the rules documented in BUG-028 deeper:
//!
//! - **Acquired** in `lifecycle.rs::try_claim_spawn_slot` before the
//!   per-key CAS check. On per-key cap rejection the guard drops,
//!   automatically releasing the speculatively-acquired permit so
//!   other groups can use it.
//! - **Held for the worker's lifetime** when admission succeeds. The
//!   guard is consumed via [`SpawnPermitGuard::forget`] at the moment
//!   the worker is officially counted (after `WorkerHandle::spawn`
//!   succeeds and the handle has been pushed to `group.idle`); from
//!   that point on, accounting is tied to `group.total` instead of
//!   the guard.
//! - **Bulk-released** by [`SpawnPermitGuard::release_n`] from paths
//!   where multiple workers exit at once (the health-check reaper's
//!   `removed_count`, the shutdown drain's `idle_count`).
//!
//! This module never references [`crate::worker::pool::WorkerPool`]
//! directly — callers pass the `Arc<Semaphore>` they already hold.
//! Keeping the API surface that small lets per-task tests construct
//! a bare `Arc<Semaphore>` without standing up a full pool.

use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// One reserved slot in the pool's global-cap permit pool.
///
/// Drop releases the permit back to the semaphore. Use
/// [`Self::forget`] to detach the permit from this guard when the
/// caller is taking over lifetime tracking (e.g. when handing the
/// slot to a freshly-spawned worker whose exit path will release via
/// [`Self::release_n`] or an explicit `add_permits` call paired with
/// the worker's `group.total.fetch_sub`).
#[derive(Debug)]
pub(super) struct SpawnPermitGuard {
    permit: Option<OwnedSemaphorePermit>,
}

impl SpawnPermitGuard {
    /// Try to reserve one slot. Returns [`PermitRejected`] when the
    /// global cap is already at `max_total_workers`. This is the
    /// non-blocking entry used inside `try_claim_spawn_slot`; the
    /// awaitable counterpart lives at the dispatch.rs saturation
    /// wait, which calls `Semaphore::acquire_owned` directly.
    pub(super) fn try_acquire(sem: &Arc<Semaphore>) -> Result<Self, PermitRejected> {
        let permit = sem
            .clone()
            .try_acquire_owned()
            .map_err(|_| PermitRejected)?;
        Ok(Self {
            permit: Some(permit),
        })
    }

    /// Detach the underlying permit from this guard without releasing
    /// it. Used at the precise moment a worker becomes "officially"
    /// counted (its handle is in `group.idle` and `group.total` has
    /// been incremented): from that point onward the pool's exit
    /// paths release via `add_permits`/`release_n`, not via this
    /// guard's drop.
    pub(super) fn forget(mut self) {
        if let Some(p) = self.permit.take() {
            // `forget` consumes the permit and disables its drop
            // release; the permit *count* stays consumed on the
            // semaphore until someone calls `add_permits`.
            p.forget();
        }
    }

    /// Bulk-release `n` permits to the semaphore. Used by exit paths
    /// where the original guards were already `forget`-ed and we are
    /// settling the count by reading `removed_count` /
    /// `idle_count` from the per-group bookkeeping.
    pub(super) fn release_n(sem: &Semaphore, n: usize) {
        if n > 0 {
            sem.add_permits(n);
        }
    }

    /// Acquire-or-skip helper for paths that *create* a worker outside
    /// `try_claim_spawn_slot` (the reaper restart, registry
    /// discovery, warmup TCP integration). Returns `Some(guard)` to
    /// proceed with the spawn/integration; returns `None` after
    /// invoking `on_skip` for telemetry. The three call sites
    /// previously open-coded a near-identical match block; this
    /// consolidates the rejection handling so the "what to do when
    /// the global cap is reached" policy lives in one place.
    pub(super) fn try_acquire_or_skip(
        sem: &Arc<Semaphore>,
        on_skip: impl FnOnce(),
    ) -> Option<Self> {
        match Self::try_acquire(sem) {
            Ok(g) => Some(g),
            Err(_) => {
                on_skip();
                None
            }
        }
    }
}

/// Returned by [`SpawnPermitGuard::try_acquire`] when the global cap
/// is exhausted. Carries no payload — the caller logs/records via
/// `permit_rejections_total` at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("global worker cap reached")]
pub(super) struct PermitRejected;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_then_drop_releases_permit() {
        let sem = Arc::new(Semaphore::new(2));
        let g1 = SpawnPermitGuard::try_acquire(&sem).expect("first");
        assert_eq!(sem.available_permits(), 1);
        let g2 = SpawnPermitGuard::try_acquire(&sem).expect("second");
        assert_eq!(sem.available_permits(), 0);
        assert!(SpawnPermitGuard::try_acquire(&sem).is_err());
        drop(g1);
        assert_eq!(sem.available_permits(), 1);
        drop(g2);
        assert_eq!(sem.available_permits(), 2);
    }

    #[test]
    fn forget_does_not_release() {
        let sem = Arc::new(Semaphore::new(2));
        let g = SpawnPermitGuard::try_acquire(&sem).expect("acquire");
        assert_eq!(sem.available_permits(), 1);
        g.forget();
        // Permit count stays consumed by the now-absent owner; nothing
        // refunds it until release_n / add_permits.
        assert_eq!(sem.available_permits(), 1);
    }

    #[test]
    fn release_n_adds_permits() {
        let sem = Arc::new(Semaphore::new(4));
        let g1 = SpawnPermitGuard::try_acquire(&sem).unwrap();
        let g2 = SpawnPermitGuard::try_acquire(&sem).unwrap();
        // Simulate the workers' guards being detached; we now own the
        // accounting externally.
        g1.forget();
        g2.forget();
        assert_eq!(sem.available_permits(), 2);
        // External counter tells us two workers exited; refund.
        SpawnPermitGuard::release_n(&sem, 2);
        assert_eq!(sem.available_permits(), 4);
    }

    #[test]
    fn release_n_with_zero_is_noop() {
        let sem = Arc::new(Semaphore::new(3));
        SpawnPermitGuard::release_n(&sem, 0);
        assert_eq!(sem.available_permits(), 3);
    }
}
