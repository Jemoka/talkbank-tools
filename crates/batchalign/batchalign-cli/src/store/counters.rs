//! Private operational-counter storage for [`JobStore`](super::JobStore).
//!
//! These counters are mutable shared state, but they are not part of the main
//! job registry. Pulling them into their own component keeps health/metrics
//! bookkeeping from looking like just another random mutex field on the store.

use tokio::sync::Mutex;

use super::OperationalCounters;

/// Store-owned mutable operational counters.
#[derive(Debug, Default)]
pub(crate) struct OperationalCounterStore {
    counters: Mutex<OperationalCounters>,
}

impl OperationalCounterStore {
    /// Create a counter store with all counters initialized to zero.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Read one projection from the current counter snapshot.
    pub(crate) async fn inspect<R>(&self, f: impl FnOnce(&OperationalCounters) -> R) -> R {
        let counters = self.counters.lock().await;
        f(&counters)
    }

    /// Apply one in-place counter mutation.
    pub(crate) async fn mutate<R>(&self, f: impl FnOnce(&mut OperationalCounters) -> R) -> R {
        let mut counters = self.counters.lock().await;
        f(&mut counters)
    }
}
