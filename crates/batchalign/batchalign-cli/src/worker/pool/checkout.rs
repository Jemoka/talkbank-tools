//! `CheckedOutWorker` — RAII guard for dispatched workers.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::worker::handle::WorkerHandle;

use super::WorkerGroup;

/// RAII guard that owns a [`WorkerHandle`] for the duration of a dispatch.
///
/// Created by `WorkerPool::checkout()` after acquiring a semaphore permit
/// and popping a worker from the idle queue. Dereferences to `WorkerHandle`
/// so callers can call `process()`, `batch_infer()`, etc. directly.
///
/// # Drop semantics
///
/// When this guard is dropped (whether the dispatch succeeded or failed),
/// the worker is returned to the group's idle queue and a semaphore permit
/// is released, unblocking the next caller waiting in `checkout()`.
///
/// If the worker was *taken* via [`take()`](Self::take) (e.g. because it
/// died mid-dispatch and should not be reused), `total` is decremented
/// instead and no permit is released -- the worker slot is permanently
/// freed so a fresh worker can be spawned later.
pub struct CheckedOutWorker {
    /// The worker handle, wrapped in `Option` so [`take()`](Self::take)
    /// can extract it. `None` only after `take()` -- the `Deref` impl
    /// panics if accessed in this state.
    pub(super) handle: Option<WorkerHandle>,
    /// Back-reference to the group this worker belongs to, used by `Drop`
    /// to return the worker to the correct idle queue and semaphore.
    pub(super) group: Arc<WorkerGroup>,
}

impl CheckedOutWorker {
    /// Take the worker out of this guard (e.g., because it died).
    ///
    /// The taken worker will be dropped normally (triggering `WorkerHandle::Drop`
    /// which sends SIGTERM+SIGKILL). `total` is decremented and no permit is
    /// released (the worker slot is gone).
    #[allow(dead_code)]
    pub fn take(&mut self) -> Option<WorkerHandle> {
        let handle = self.handle.take()?;
        self.group
            .total
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        // The worker is gone; refund the global-cap admission slot
        // so a fresh worker (this key or another) can take its
        // place.
        self.group.spawn_permits.add_permits(1);
        Some(handle)
    }
}

/// # Panics
///
/// Panics if the handle has been removed via [`CheckedOutWorker::take()`].
/// This is a programming error -- callers must not dereference a guard
/// after taking the worker out. The `Deref` trait cannot return `Result`,
/// so a panic is the only signal available.
impl Deref for CheckedOutWorker {
    type Target = WorkerHandle;
    fn deref(&self) -> &WorkerHandle {
        // Caller-contract invariant (see doc comment above): callers
        // must not dereference a guard after `.take()`. The `Deref`
        // trait cannot return `Result`, so a panic is the only signal
        // available. Reaching this expect indicates a bug in the
        // calling code, not a recoverable runtime condition.
        #[allow(clippy::expect_used)]
        self.handle.as_ref().expect(
            "BUG: CheckedOutWorker dereferenced after take() -- \
             the worker handle has been consumed and is no longer available",
        )
    }
}

/// # Panics
///
/// Panics if the handle has been removed via [`CheckedOutWorker::take()`].
/// See [`Deref`] impl for rationale.
impl DerefMut for CheckedOutWorker {
    fn deref_mut(&mut self) -> &mut WorkerHandle {
        // Same caller-contract invariant as `Deref::deref` above.
        #[allow(clippy::expect_used)]
        self.handle.as_mut().expect(
            "BUG: CheckedOutWorker dereferenced after take() -- \
             the worker handle has been consumed and is no longer available",
        )
    }
}

impl Drop for CheckedOutWorker {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Return the worker to the idle queue and release a permit.
            super::lock_recovered(&self.group.idle).push_back(handle);
            self.group.available.add_permits(1);
        }
        // If handle was `None` (taken via `take()`), total was already
        // decremented -- nothing to do.

        // Wake ONE task parked on `WorkerPool::worker_returned` —
        // typically a cross-key spawn attempt waiting for an eviction
        // opportunity. FIFO-fair: each worker return wakes exactly
        // one waiter, eliminating the thundering-herd re-probe storm
        // documented in BUG-028. If the woken waiter's key turns out
        // to be uneviable for this particular return, it re-parks on
        // the same Notify and the next return wakes the next-in-line
        // waiter. Bounded retry is enforced by the dispatch slow
        // path's `wait_deadline`.
        self.group.worker_returned.notify_one();
    }
}
