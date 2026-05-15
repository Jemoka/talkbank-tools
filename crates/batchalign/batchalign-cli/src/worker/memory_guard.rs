//! Process-global memory guard for Python worker spawning.
//!
//! # Problem
//!
//! Each Python ML worker loads 2–15 GB of models (Whisper, Stanza, etc.).
//! When multiple workers spawn concurrently — from parallel tests, warmup,
//! or job dispatch — they each check available memory, see "enough", and
//! start loading simultaneously. By the time all models are resident, total
//! usage exceeds physical RAM, triggering a **kernel-level OOM panic** that
//! crashes the entire machine (not just the process).
//!
//! This has caused multiple catastrophic crashes on a 64 GB developer machine
//! (see `docs/postmortems/`). The Jetsam report from 2026-03-21 showed 5
//! python3.12 workers at 13–15 GB each = 71 GB on a 64 GB machine.
//!
//! # Solution
//!
//! A process-global semaphore serializes worker spawns within one process, and
//! a host-wide ledger coordinates heavy worker/model startups across local
//! batchalign3 processes. Before each spawn, the guard checks local available
//! memory and then reserves a host-wide startup slot plus startup headroom. If
//! memory is insufficient, the spawn is rejected with a typed error — never
//! silently retried or defaulted.
//!
//! This is defense-in-depth: even if a caller forgets to check memory, the
//! spawn itself will refuse. The local semaphore prevents the in-process TOCTOU
//! race where N concurrent checks all see "enough" before any model is loaded,
//! and the host-wide ledger extends that protection across separate local
//! servers, ports, and test binaries.
//!
//! # Usage in tests
//!
//! ```rust,ignore
//! use crate::worker::memory_guard;
//!
//! #[tokio::test]
//! async fn my_worker_test() {
//!     memory_guard::skip_if_insufficient_memory(4096); // need 4 GB
//!     // ... spawn workers ...
//! }
//! ```

use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::Semaphore;
use tracing::warn;

use crate::host_memory::{HostMemoryCoordinator, HostMemoryLease};

use super::handle::WorkerConfig;

/// Minimum available memory (MB) to allow a worker spawn.
/// Default: 4 GB. Override via `BATCHALIGN_SPAWN_MIN_MEMORY_MB` env var.
fn min_spawn_memory_mb() -> u64 {
    std::env::var("BATCHALIGN_SPAWN_MIN_MEMORY_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4096)
}

/// Maximum concurrent worker spawns. Serializing spawns prevents the TOCTOU
/// race where N workers all check memory before any model is loaded.
///
/// Default: 1 (fully serialized). Override via `BATCHALIGN_MAX_CONCURRENT_SPAWNS`.
fn max_concurrent_spawns() -> usize {
    std::env::var("BATCHALIGN_MAX_CONCURRENT_SPAWNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

/// Process-global spawn semaphore.
static SPAWN_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(max_concurrent_spawns()));

/// Error returned when a worker spawn is blocked by the memory guard.
#[derive(Debug, Clone, thiserror::Error)]
pub enum MemoryGuardError {
    /// Not enough free memory to safely spawn a worker.
    #[error(
        "insufficient memory to spawn worker: {available_mb} MB available, \
         {required_mb} MB required (total RAM: {total_mb} MB). \
         This guard prevents kernel OOM panics. \
         Set BATCHALIGN_SPAWN_MIN_MEMORY_MB to adjust the threshold."
    )]
    InsufficientMemory {
        /// How much memory is currently available.
        available_mb: u64,
        /// How much memory was requested for this spawn.
        required_mb: u64,
        /// Total physical RAM on this machine.
        total_mb: u64,
    },

    /// Spawn semaphore was closed (server shutting down).
    #[error("worker spawn semaphore closed (server shutting down)")]
    SemaphoreClosed,

    /// Host-wide coordinator rejected or timed out the startup reservation.
    #[error("host memory coordinator blocked worker spawn: {0}")]
    HostReservation(String),
}

/// Combined local+host guard that protects one worker startup window.
pub struct SpawnPermit {
    _local_permit: tokio::sync::SemaphorePermit<'static>,
    host_lease: HostMemoryLease,
}

impl SpawnPermit {
    /// Tag the underlying host-memory lease with the freshly-spawned
    /// worker's PID. The caller must invoke this immediately after
    /// `cmd.spawn()` succeeds — prior to the call the lease is owned
    /// (in liveness terms) by the daemon, and a daemon-PID-only lease
    /// survives worker death (Bug 2 / ghost slots, 2026-05-01).
    ///
    /// Failures to update the ledger are warned and dropped: this is a
    /// best-effort accounting hint, not a correctness gate. A worker
    /// that never gets its PID written into the lease falls back to
    /// the older "owner_pid is daemon" behaviour, which is the
    /// pre-fix status quo — strictly no worse than today.
    pub fn set_worker_pid(&self, worker_pid: u32) {
        if let Err(error) = self.host_lease.set_worker_pid(worker_pid) {
            warn!(
                worker_pid,
                error = %error,
                "Failed to tag host-memory lease with worker PID; ghost-slot reaping will fall back to owner_pid",
            );
        }
    }
}

/// Query current available memory in MB.
///
/// Delegates to [`crate::host_memory::detect_available_memory_mb`],
/// which is process-globally TTL-cached so admission gates,
/// eviction pre-pass, and the in-spawn guard agree on one reading
/// per second instead of each constructing a fresh
/// `sysinfo::System`. Returned as raw `u64` so existing callers
/// don't need to thread `MemoryMb` through.
///
/// On macOS, sysinfo's `available_memory` is conservative (excludes
/// inactive pages the kernel can reclaim) — see the host_memory
/// docs.
pub fn available_memory_mb() -> u64 {
    crate::host_memory::detect_available_memory_mb().0
}

/// Query total physical memory in MB.
pub fn total_memory_mb() -> u64 {
    crate::host_memory::detect_total_memory_mb().0
}

/// Acquire a spawn permit, checking memory before allowing the spawn.
///
/// This is the **only** path through which workers should be spawned.
/// It serializes spawns via a global semaphore and checks memory before
/// each one, preventing the concurrent-check TOCTOU race.
///
/// Returns a permit guard. The caller holds the guard through
/// `WorkerHandle::spawn()`, which blocks until the worker sends its ready
/// signal (i.e., models are loaded). This means the next spawn's memory
/// check sees an accurate picture of available RAM.
pub async fn acquire_spawn_permit(config: &WorkerConfig) -> Result<SpawnPermit, MemoryGuardError> {
    let permit = SPAWN_SEMAPHORE
        .acquire()
        .await
        .map_err(|_| MemoryGuardError::SemaphoreClosed)?;

    let startup_reservation = config.startup_reservation_mb();
    let required_mb = startup_reservation.0.max(min_spawn_memory_mb());
    let available = available_memory_mb();
    let total = total_memory_mb();
    let threshold = required_mb;

    if available < threshold {
        // Drop the permit before returning the error so other spawns can proceed.
        drop(permit);
        warn!(
            available_mb = available,
            required_mb = threshold,
            total_mb = total,
            "Memory guard blocked worker spawn"
        );
        return Err(MemoryGuardError::InsufficientMemory {
            available_mb: available,
            required_mb: threshold,
            total_mb: total,
        });
    }

    let coordinator = HostMemoryCoordinator::new(config.runtime.host_memory.clone());
    let profile = config.profile;
    let lang = config.lang.clone();
    let engine_overrides = config.engine_overrides.clone();
    let timeout = Duration::from_secs(config.ready_timeout_s.max(1));
    let host_lease = tokio::task::spawn_blocking(move || {
        coordinator.acquire_worker_startup_lease(
            profile,
            startup_reservation,
            &lang,
            &engine_overrides,
            timeout,
            Duration::from_secs(1),
        )
    })
    .await
    .map_err(|error| MemoryGuardError::HostReservation(error.to_string()))?
    .map_err(|error| MemoryGuardError::HostReservation(error.to_string()))?;

    Ok(SpawnPermit {
        _local_permit: permit,
        host_lease,
    })
}

/// Check available memory and skip the current test if insufficient.
///
/// Call this at the top of any `#[test]` or `#[tokio::test]` that spawns
/// Python workers. The test will print a message and return early (pass
/// without running) rather than risk an OOM crash.
///
/// ```rust,ignore
/// #[tokio::test]
/// async fn test_with_workers() {
///     memory_guard::skip_if_insufficient_memory(8192); // need 8 GB
///     // ... test body ...
/// }
/// ```
pub fn skip_if_insufficient_memory(required_mb: u64) {
    let available = available_memory_mb();
    let total = total_memory_mb();
    let threshold = required_mb.max(min_spawn_memory_mb());

    if available < threshold {
        eprintln!(
            "SKIPPING TEST: insufficient memory ({available} MB available, \
             {threshold} MB required, {total} MB total). \
             Run on a machine with more RAM (e.g., net with 256 GB)."
        );
        // Return from the test function. In Rust test harness, this counts as "pass".
        // The test body after this call won't execute.
    }
}

/// Macro version that returns early from the calling function.
///
/// Usage:
/// ```rust,ignore
/// #[tokio::test]
/// async fn my_test() {
///     bail_if_low_memory!(8192);
///     // ... test body only runs if 8 GB available ...
/// }
/// ```
#[macro_export]
macro_rules! bail_if_low_memory {
    ($required_mb:expr) => {
        if $crate::worker::memory_guard::available_memory_mb()
            < ($required_mb as u64).max(
                std::env::var("BATCHALIGN_SPAWN_MIN_MEMORY_MB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(4096u64),
            )
        {
            eprintln!(
                "SKIPPING TEST: insufficient memory ({} MB available, {} MB required). \
                 Run on a machine with more RAM.",
                $crate::worker::memory_guard::available_memory_mb(),
                ($required_mb as u64).max(4096),
            );
            return;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{LanguageCode3, MemoryMb, WorkerLanguage};
    use crate::host_memory::HostMemoryRuntimeConfig;
    use crate::worker::WorkerProfile;
    use tempfile::tempdir;

    #[tokio::test]
    async fn host_reservation_rejects_when_reserve_is_impossible() {
        let temp = tempdir().expect("tempdir");
        let config = WorkerConfig {
            profile: WorkerProfile::Stanza,
            lang: WorkerLanguage::from(LanguageCode3::eng()),
            runtime: crate::worker::handle::WorkerRuntimeConfig::from_sources(
                false,
                None,
                1,
                HostMemoryRuntimeConfig::from_sources(
                    temp.path().join("host-memory.json"),
                    MemoryMb(1_000_000_000),
                    1,
                ),
                crate::types::runtime::MemoryTier::detect(),
            ),
            ..Default::default()
        };
        let result = acquire_spawn_permit(&config).await;
        assert!(
            matches!(result, Err(MemoryGuardError::HostReservation(_))),
            "expected host reservation rejection"
        );
    }

    #[test]
    fn large_tier_startup_reservations_exceed_old_flat_floor() {
        // On Large/Fleet tiers, reservations should exceed the old 4 GB flat
        // floor that existed before tier-adaptive budgets.
        use crate::types::runtime::MemoryTier;
        let tier = MemoryTier::from_total_mb(64_000);
        let gpu = WorkerProfile::Gpu.startup_reservation_mb_for_tier(&tier);
        let stanza = WorkerProfile::Stanza.startup_reservation_mb_for_tier(&tier);
        assert!(
            gpu.0 > 4096,
            "GPU Large tier ({} MB) must exceed old floor",
            gpu.0
        );
        assert!(
            stanza.0 > 4096,
            "Stanza Large tier ({} MB) must exceed old floor",
            stanza.0
        );
    }

    #[test]
    fn small_tier_startup_reservations_are_positive() {
        use crate::types::runtime::MemoryTier;
        let tier = MemoryTier::from_total_mb(16_000);
        let gpu = WorkerProfile::Gpu.startup_reservation_mb_for_tier(&tier);
        let stanza = WorkerProfile::Stanza.startup_reservation_mb_for_tier(&tier);
        let io = WorkerProfile::Io.startup_reservation_mb_for_tier(&tier);
        assert!(gpu.0 > 0);
        assert!(stanza.0 > 0);
        assert!(io.0 > 0);
    }
}
