//! Host-wide memory coordination across local batchalign3 processes.
//!
//! The Rust server can be only one memory consumer on a machine that also runs
//! other batchalign3 ports, CLI daemons, tests, or unrelated inference tools.
//! This module provides a small machine-local coordination ledger so
//! participating batchalign3 processes can serialize heavy worker startups and
//! conservatively reserve job execution memory on shared hosts.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessesToUpdate, System};
use uuid::Uuid;

use crate::api::{MemoryMb, NumWorkers, ReleasedCommand, WorkerLanguage};
use crate::config::ServerConfig;
use crate::runtime;
use crate::worker::WorkerProfile;

const DEFAULT_LOCK_POLL: Duration = Duration::from_secs(1);
const DEFAULT_TEST_LOCK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Host-wide memory pressure level derived from the current memory snapshot and
/// reserved headroom.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum HostMemoryPressureLevel {
    /// Plenty of free headroom remains after the configured reserve.
    #[default]
    Healthy,
    /// Some headroom remains, but operators should expect reduced concurrency.
    Guarded,
    /// Very little headroom remains; only small new reservations should fit.
    Constrained,
    /// The configured reserve is exhausted or nearly exhausted.
    Critical,
}

/// Runtime-owned configuration for the machine-local memory ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMemoryRuntimeConfig {
    /// Shared ledger path used by local batchalign3 processes on this host.
    pub coordinator_path: PathBuf,
    /// Minimum free memory to preserve after reservations are granted.
    pub reserve_mb: MemoryMb,
    /// Maximum concurrent worker/model startups allowed across the host.
    pub max_concurrent_worker_startups: usize,
}

impl HostMemoryRuntimeConfig {
    /// Build runtime config from server-level settings.
    pub fn from_server_config(config: &ServerConfig) -> Self {
        Self {
            coordinator_path: default_host_memory_ledger_path(),
            reserve_mb: config.resolved_memory_gate_mb(),
            max_concurrent_worker_startups: config.max_concurrent_worker_startups as usize,
        }
    }

    /// Build runtime config from explicit sources.
    pub fn from_sources(
        coordinator_path: PathBuf,
        reserve_mb: MemoryMb,
        max_concurrent_worker_startups: usize,
    ) -> Self {
        Self {
            coordinator_path,
            reserve_mb,
            max_concurrent_worker_startups: max_concurrent_worker_startups.max(1),
        }
    }
}

impl Default for HostMemoryRuntimeConfig {
    fn default() -> Self {
        Self {
            coordinator_path: default_host_memory_ledger_path(),
            // `Default::default()` returns the post-resolution value
            // so callers (mainly tests) get a sensible reserve without
            // building a `ServerConfig` themselves.
            reserve_mb: ServerConfig::default().resolved_memory_gate_mb(),
            max_concurrent_worker_startups: ServerConfig::default().max_concurrent_worker_startups
                as usize,
        }
    }
}

/// Default machine-local ledger path for host memory coordination.
pub fn default_host_memory_ledger_path() -> PathBuf {
    if let Some(explicit) = std::env::var_os("BATCHALIGN_HOST_MEMORY_LEDGER") {
        return PathBuf::from(explicit);
    }
    let suffix = host_ledger_suffix();
    std::env::temp_dir().join(format!("batchalign3-host-memory-{suffix}.json"))
}

/// Test-only: redirect the host-memory ledger to a per-process tempdir
/// path so concurrent integration-test binaries don't race on the
/// shared default file. Idempotent (`Once`-guarded); safe to call from
/// any test entry point that constructs a `HostMemoryRuntimeConfig`.
///
/// Why this lives in the production crate: integration-test sibling
/// modules (`tests/cli_common`, `tests/common/test_server_fixture`,
/// the per-binary `require_python!` macros) all need this hook before
/// any `WorkerHandle::spawn` or `prepare_workers` runs. Rust's test
/// crate model doesn't let those siblings import each other, so a
/// shared library function is the only way to de-duplicate.
#[doc(hidden)]
pub fn isolate_host_memory_ledger_for_test() {
    use std::sync::Once;
    static SET_LEDGER: Once = Once::new();
    SET_LEDGER.call_once(|| {
        let pid = std::process::id();
        let ledger =
            std::env::temp_dir().join(format!("batchalign3-host-memory-test-fixture-{pid}.json"));
        let _ = std::fs::remove_file(&ledger);
        // Safety: Once::call_once gates this so no concurrent reader of
        // BATCHALIGN_HOST_MEMORY_LEDGER exists at this point. Setting
        // the var here, before any HostMemoryRuntimeConfig is built,
        // means every later read sees the per-process value.
        unsafe {
            std::env::set_var("BATCHALIGN_HOST_MEMORY_LEDGER", &ledger);
        }
    });
}

/// Summary of the current host-memory coordination state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMemorySnapshot {
    /// Total physical memory observed by the OS.
    pub total_mb: MemoryMb,
    /// Currently available memory observed by the OS.
    pub available_mb: MemoryMb,
    /// Reserved low-water-mark that the coordinator keeps free.
    pub reserve_mb: MemoryMb,
    /// Sum of active reservation amounts recorded in the ledger.
    pub active_reserved_mb: MemoryMb,
    /// Number of active startup leases.
    pub startup_leases: usize,
    /// Number of active job-execution leases.
    pub job_execution_leases: usize,
    /// Number of active machine-wide ML test locks.
    pub ml_test_locks: usize,
    /// Human-readable lease labels for operator debugging.
    pub active_lease_labels: Vec<String>,
    /// Pressure level derived from the snapshot.
    pub pressure_level: HostMemoryPressureLevel,
}

/// One acquired host-memory lease. Releasing the lease removes it from the
/// machine-local ledger.
pub struct HostMemoryLease {
    ledger_path: PathBuf,
    lease_id: String,
    released: bool,
}

impl HostMemoryLease {
    fn new(ledger_path: PathBuf, lease_id: String) -> Self {
        Self {
            ledger_path,
            lease_id,
            released: false,
        }
    }

    /// Release the lease immediately.
    pub fn release(mut self) {
        self.release_internal();
    }

    /// Tag this lease with the worker subprocess PID it represents.
    ///
    /// Why this exists: when the daemon acquires a worker-startup lease,
    /// it doesn't yet have the worker's PID — the spawn happens after
    /// the reservation is granted. Once `Child::id()` is available, the
    /// caller calls `set_worker_pid(child_pid)` so the lease record
    /// records the worker, not the daemon. From that point on,
    /// `prune_stale_leases` uses the worker's liveness to decide whether
    /// to retain the lease — closing the Bug 2 (ghost-slot) accounting
    /// drift that wedged ming on 2026-05-01.
    ///
    /// Returns `Ok(())` on successful update, or `HostMemoryError` if
    /// the ledger could not be locked or rewritten. The lease's
    /// `released` state is unchanged.
    pub fn set_worker_pid(&self, worker_pid: u32) -> Result<(), HostMemoryError> {
        let path = self.ledger_path.clone();
        let lease_id = self.lease_id.clone();
        with_locked_ledger(&path, move |ledger: &mut MemoryLedger| {
            for lease in ledger.leases.iter_mut() {
                if lease.id == lease_id {
                    lease.worker_pid = Some(worker_pid);
                    return Ok(());
                }
            }
            // Lease was already pruned (e.g., daemon-side bookkeeping
            // already dropped it). Treat as a no-op rather than an
            // error — the caller's intent (associate this lease with a
            // worker PID) is satisfied vacuously.
            Ok(())
        })
    }

    fn release_internal(&mut self) {
        if self.released {
            return;
        }
        let path = self.ledger_path.clone();
        let lease_id = self.lease_id.clone();
        let _ = with_locked_ledger(&path, |ledger| {
            ledger.leases.retain(|lease| lease.id != lease_id);
            Ok(())
        });
        self.released = true;
    }
}

impl Drop for HostMemoryLease {
    fn drop(&mut self) {
        self.release_internal();
    }
}

/// A machine-local exclusive lock for real-model ML tests.
pub struct MachineMlTestLock {
    _lease: HostMemoryLease,
}

impl MachineMlTestLock {
    /// Acquire the machine-wide ML test lock, waiting until other local test
    /// binaries release it.
    pub fn acquire(label: &str) -> Result<Self, HostMemoryError> {
        let coordinator = HostMemoryCoordinator::new(HostMemoryRuntimeConfig::default());
        let lease = coordinator.acquire_ml_test_lock(
            label,
            DEFAULT_TEST_LOCK_TIMEOUT,
            DEFAULT_LOCK_POLL,
        )?;
        Ok(Self { _lease: lease })
    }
}

/// Result of planning one job's host-memory execution reservation.
pub struct JobExecutionPlan {
    /// File-level worker count granted for this job under current host pressure.
    pub granted_workers: NumWorkers,
    /// Original requested worker count before host-memory clamping.
    pub requested_workers: NumWorkers,
    /// Reservation held for the duration of the job.
    pub lease: HostMemoryLease,
    /// Total reserved memory for the job execution window.
    pub reserved_mb: MemoryMb,
}

/// Errors raised by the host-memory coordinator.
#[derive(Debug, thiserror::Error)]
pub enum HostMemoryError {
    /// Failed to read or write the machine-local ledger.
    #[error("host-memory ledger I/O failed at {path}: {source}")]
    Io {
        /// Path to the shared ledger file.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// The ledger file exists but cannot be parsed.
    #[error("host-memory ledger is corrupt at {path}: {message}")]
    CorruptLedger {
        /// Path to the shared ledger file.
        path: PathBuf,
        /// Human-readable parse failure.
        message: String,
    },
    /// The host reserve would be exceeded by the requested reservation.
    #[error(
        "host-memory reserve would be exceeded for {label}: {available_mb} MB available, \
         {pending_reserved_mb} MB already reserved, {requested_mb} MB requested, \
         {reserve_mb} MB reserved for host headroom (total RAM: {total_mb} MB)"
    )]
    CapacityRejected {
        /// Human-readable request label.
        label: String,
        /// Current available memory from the OS snapshot.
        available_mb: u64,
        /// Sum of active reservation amounts in the ledger.
        pending_reserved_mb: u64,
        /// Requested additional reservation.
        requested_mb: u64,
        /// Configured reserve that must remain free.
        reserve_mb: u64,
        /// Total physical RAM observed on this machine.
        total_mb: u64,
    },
    /// Another local process is already using the exclusive ML test lock.
    #[error("machine-wide ML test lock is already held by {holders:?}")]
    MlTestLockBusy {
        /// Labels of active lock holders.
        holders: Vec<String>,
    },
    /// The host-wide startup limit is currently saturated.
    #[error(
        "worker startup slots busy for {label}: {active_slots}/{max_slots} local startup slots in use"
    )]
    StartupSlotsBusy {
        /// Human-readable request label.
        label: String,
        /// Number of active startup slots.
        active_slots: usize,
        /// Configured host-wide startup slot limit.
        max_slots: usize,
    },
    /// Waiting for capacity exceeded the configured timeout.
    #[error(
        "timed out waiting for host-memory capacity for {label} after {waited_s}s: {last_reason}"
    )]
    TimedOut {
        /// Human-readable request label.
        label: String,
        /// Timeout window in seconds.
        waited_s: u64,
        /// Last rejection reason observed while waiting.
        last_reason: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MemoryLeaseKind {
    WorkerStartup,
    JobExecution,
    MlTestExclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryLeaseRecord {
    id: String,
    kind: MemoryLeaseKind,
    /// PID of the process that requested this lease (the daemon, in
    /// production). Liveness-checked when `worker_pid` is `None`.
    owner_pid: u32,
    /// Optional PID of the worker subprocess whose memory the lease
    /// represents. When `Some`, prune_stale_leases checks THIS PID's
    /// liveness instead of `owner_pid`. The daemon spawns workers in
    /// their own process group, so a worker can die independently of
    /// the daemon — Bug 2 (ghost slots) was the daemon's reservation
    /// surviving the worker's death because the lease was tagged with
    /// the (always-alive) daemon PID. Tagging with the worker's PID
    /// instead lets the reaper detect the dead worker and reclaim the
    /// slot. `#[serde(default)]` keeps backward compatibility with
    /// older ledger files that predate this field.
    #[serde(default)]
    worker_pid: Option<u32>,
    reserved_mb: u64,
    startup_slot: bool,
    label: String,
    created_at_epoch_s: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MemoryLedger {
    leases: Vec<MemoryLeaseRecord>,
}

/// Per-owner `max(sum(JobExecution), sum(WorkerStartup))`, summed over
/// owners. `WorkerStartup` is a sub-allocation of `JobExecution` for
/// the same owner (Contract D), so the larger of the two surfaces the
/// owner's actual pending demand. `MlTestExclusive` carries 0 MB and
/// folds into the `JobExecution` half.
fn effective_reserved_mb(leases: &[MemoryLeaseRecord]) -> u64 {
    use std::collections::HashMap;
    let mut by_owner: HashMap<u32, (u64, u64)> = HashMap::new();
    for lease in leases {
        let entry = by_owner.entry(lease.owner_pid).or_default();
        match lease.kind {
            MemoryLeaseKind::JobExecution | MemoryLeaseKind::MlTestExclusive => {
                entry.0 = entry.0.saturating_add(lease.reserved_mb);
            }
            MemoryLeaseKind::WorkerStartup => {
                entry.1 = entry.1.saturating_add(lease.reserved_mb);
            }
        }
    }
    by_owner.values().map(|(je, ws)| (*je).max(*ws)).sum()
}

#[derive(Debug, Clone)]
struct MemoryLeaseRequest {
    kind: MemoryLeaseKind,
    reserved_mb: MemoryMb,
    startup_slot: bool,
    label: String,
}

#[derive(Debug, Clone, Copy)]
struct SystemMemorySnapshot {
    total_mb: MemoryMb,
    available_mb: MemoryMb,
}

/// Machine-local coordinator that serializes access to the shared memory ledger.
#[derive(Debug, Clone)]
pub struct HostMemoryCoordinator {
    config: HostMemoryRuntimeConfig,
}

impl HostMemoryCoordinator {
    /// Create one host-memory coordinator from runtime config.
    pub fn new(config: HostMemoryRuntimeConfig) -> Self {
        Self { config }
    }

    /// Build one coordinator from server settings.
    pub fn from_server_config(config: &ServerConfig) -> Self {
        Self::new(HostMemoryRuntimeConfig::from_server_config(config))
    }

    /// Return the shared ledger path backing this coordinator.
    pub fn coordinator_path(&self) -> &Path {
        &self.config.coordinator_path
    }

    /// Return the current host-memory snapshot for health/status reporting.
    pub fn snapshot(&self) -> Result<HostMemorySnapshot, HostMemoryError> {
        with_locked_ledger(&self.config.coordinator_path, |ledger| {
            let system = system_memory_snapshot();
            let active_reserved_mb = effective_reserved_mb(&ledger.leases);
            let startup_leases = ledger
                .leases
                .iter()
                .filter(|lease| lease.kind == MemoryLeaseKind::WorkerStartup)
                .count();
            let job_execution_leases = ledger
                .leases
                .iter()
                .filter(|lease| lease.kind == MemoryLeaseKind::JobExecution)
                .count();
            let ml_test_locks = ledger
                .leases
                .iter()
                .filter(|lease| lease.kind == MemoryLeaseKind::MlTestExclusive)
                .count();
            let active_lease_labels = ledger
                .leases
                .iter()
                .map(|lease| format!("{:?}:{}:{}MB", lease.kind, lease.label, lease.reserved_mb))
                .collect();
            Ok(HostMemorySnapshot {
                total_mb: system.total_mb,
                available_mb: system.available_mb,
                reserve_mb: self.config.reserve_mb,
                active_reserved_mb: MemoryMb(active_reserved_mb),
                startup_leases,
                job_execution_leases,
                ml_test_locks,
                active_lease_labels,
                pressure_level: pressure_level_for(system.available_mb, self.config.reserve_mb),
            })
        })
    }

    /// Wait for a host-memory startup reservation for one worker/model load.
    pub fn acquire_worker_startup_lease(
        &self,
        profile: WorkerProfile,
        startup_reservation_mb: MemoryMb,
        lang: &WorkerLanguage,
        engine_overrides: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<HostMemoryLease, HostMemoryError> {
        let request = MemoryLeaseRequest {
            kind: MemoryLeaseKind::WorkerStartup,
            reserved_mb: startup_reservation_mb,
            startup_slot: true,
            label: format!(
                "worker-startup:{}:{}:{}",
                profile.label(),
                lang,
                engine_overrides
            ),
        };
        self.wait_for_lease(request, timeout, poll_interval)
    }

    /// Acquire a machine-wide exclusive lock for real-model ML tests.
    pub fn acquire_ml_test_lock(
        &self,
        label: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<HostMemoryLease, HostMemoryError> {
        let request = MemoryLeaseRequest {
            kind: MemoryLeaseKind::MlTestExclusive,
            reserved_mb: MemoryMb(0),
            startup_slot: false,
            label: label.to_owned(),
        };
        self.wait_for_lease(request, timeout, poll_interval)
    }

    /// Plan and reserve one job's execution memory window.
    ///
    /// The coordinator may reduce the requested worker count when host pressure
    /// is high. The returned lease stays alive for the whole job so other local
    /// processes see the reservation and conservatively back off.
    pub fn wait_for_job_execution_plan(
        &self,
        command: ReleasedCommand,
        requested_workers: NumWorkers,
        label: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<JobExecutionPlan, HostMemoryError> {
        let deadline = Instant::now() + timeout;
        let mut last_reason = String::from("no capacity decision yet");
        loop {
            match self.try_plan_job_execution(command, requested_workers, label) {
                Ok(plan) => return Ok(plan),
                Err(error)
                    if timeout > Duration::ZERO
                        && Instant::now() < deadline
                        && retryable_error(&error) =>
                {
                    last_reason = error.to_string();
                    std::thread::sleep(poll_interval);
                }
                Err(error) => {
                    if timeout > Duration::ZERO && Instant::now() >= deadline {
                        return Err(HostMemoryError::TimedOut {
                            label: label.to_owned(),
                            waited_s: timeout.as_secs(),
                            last_reason,
                        });
                    }
                    return Err(error);
                }
            }
        }
    }

    fn try_plan_job_execution(
        &self,
        command: ReleasedCommand,
        requested_workers: NumWorkers,
        label: &str,
    ) -> Result<JobExecutionPlan, HostMemoryError> {
        with_locked_ledger(&self.config.coordinator_path, |ledger| {
            let system = system_memory_snapshot();
            // Tier-aware envelope: clamp the fleet-conservative
            // worst-case to this host's per-profile startup
            // reservation. Without the clamp, a 16 GB host that
            // asks for one morphotag worker reserves a 12 GB
            // envelope, the worker-startup lease check refuses
            // every spawn, and the pool stays dead-on-arrival.
            // memory_gate.rs admission and memory_guard.rs
            // in-spawn remain the safety net for actual at-spawn
            // OOM. See `batchalign_types::command_spec::CommandSpec`
            // and `batchalign_types::memory::estimate_per_worker_peak_mb_with_profile`.
            let tier = runtime::MemoryTier::from_total_mb(system.total_mb.0);
            let spec = batchalign_types::command_spec::command_spec_for(command);
            let runtime_is_free_threaded = runtime::is_free_threaded_runtime();
            let base = spec.base_mb_for_runtime(runtime_is_free_threaded);
            let worst_case_mb = (base.0 as f64 * spec.loading_overhead.0) as u64;
            let worst_case = batchalign_types::api::MemoryMb(worst_case_mb);
            let per_worker_budget =
                batchalign_types::memory::estimate_per_worker_peak_mb_with_profile(
                    worst_case,
                    spec.profile,
                    &tier,
                );
            let pending_reserved_mb = effective_reserved_mb(&ledger.leases);
            let Some((granted_workers, reserved_mb)) = plan_job_reservation(
                requested_workers.0,
                per_worker_budget.0,
                system.available_mb.0,
                self.config.reserve_mb.0,
                pending_reserved_mb,
            ) else {
                return Err(HostMemoryError::CapacityRejected {
                    label: label.to_owned(),
                    available_mb: system.available_mb.0,
                    pending_reserved_mb,
                    requested_mb: per_worker_budget
                        .0
                        .saturating_mul(requested_workers.0 as u64),
                    reserve_mb: self.config.reserve_mb.0,
                    total_mb: system.total_mb.0,
                });
            };

            let record = MemoryLeaseRecord {
                id: Uuid::new_v4().to_string(),
                kind: MemoryLeaseKind::JobExecution,
                owner_pid: std::process::id(),
                worker_pid: None,
                reserved_mb,
                startup_slot: false,
                label: label.to_owned(),
                created_at_epoch_s: unix_epoch_s(),
            };
            let lease =
                HostMemoryLease::new(self.config.coordinator_path.clone(), record.id.clone());
            ledger.leases.push(record);
            Ok(JobExecutionPlan {
                granted_workers: NumWorkers(granted_workers),
                requested_workers,
                lease,
                reserved_mb: MemoryMb(reserved_mb),
            })
        })
    }

    fn wait_for_lease(
        &self,
        request: MemoryLeaseRequest,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<HostMemoryLease, HostMemoryError> {
        let deadline = Instant::now() + timeout;
        let mut last_reason = String::from("no capacity decision yet");
        loop {
            match self.try_acquire_lease(request.clone()) {
                Ok(lease) => return Ok(lease),
                Err(error)
                    if timeout > Duration::ZERO
                        && Instant::now() < deadline
                        && retryable_error(&error) =>
                {
                    last_reason = error.to_string();
                    std::thread::sleep(poll_interval);
                }
                Err(error) => {
                    if timeout > Duration::ZERO && Instant::now() >= deadline {
                        return Err(HostMemoryError::TimedOut {
                            label: request.label,
                            waited_s: timeout.as_secs(),
                            last_reason,
                        });
                    }
                    return Err(error);
                }
            }
        }
    }

    fn try_acquire_lease(
        &self,
        request: MemoryLeaseRequest,
    ) -> Result<HostMemoryLease, HostMemoryError> {
        with_locked_ledger(&self.config.coordinator_path, |ledger| {
            if request.kind == MemoryLeaseKind::MlTestExclusive {
                let holders: Vec<String> = ledger
                    .leases
                    .iter()
                    .filter(|lease| lease.kind == MemoryLeaseKind::MlTestExclusive)
                    .map(|lease| lease.label.clone())
                    .collect();
                if !holders.is_empty() {
                    return Err(HostMemoryError::MlTestLockBusy { holders });
                }
            }

            if request.startup_slot {
                let active_slots = ledger
                    .leases
                    .iter()
                    .filter(|lease| lease.startup_slot)
                    .count();
                if active_slots >= self.config.max_concurrent_worker_startups {
                    return Err(HostMemoryError::StartupSlotsBusy {
                        label: request.label.clone(),
                        active_slots,
                        max_slots: self.config.max_concurrent_worker_startups,
                    });
                }
            }

            let system = system_memory_snapshot();
            let pending_reserved_mb = effective_reserved_mb(&ledger.leases);
            // Project what `effective_reserved_mb` would be AFTER admitting
            // the new lease. We simulate by appending a placeholder lease
            // record with the same `(kind, owner_pid, reserved_mb)` and
            // recomputing — Contract D semantics (sub-allocation) then
            // apply uniformly to the new lease too.
            let projected_reserved_mb = {
                let mut simulated: Vec<MemoryLeaseRecord> =
                    Vec::with_capacity(ledger.leases.len() + 1);
                simulated.extend(ledger.leases.iter().cloned());
                simulated.push(MemoryLeaseRecord {
                    id: String::new(),
                    kind: request.kind,
                    owner_pid: std::process::id(),
                    worker_pid: None,
                    reserved_mb: request.reserved_mb.0,
                    startup_slot: request.startup_slot,
                    label: String::new(),
                    created_at_epoch_s: 0,
                });
                effective_reserved_mb(&simulated)
            };
            let projected_available_mb =
                system.available_mb.0.saturating_sub(projected_reserved_mb);
            if projected_available_mb < self.config.reserve_mb.0 {
                return Err(HostMemoryError::CapacityRejected {
                    label: request.label.clone(),
                    available_mb: system.available_mb.0,
                    pending_reserved_mb,
                    requested_mb: request.reserved_mb.0,
                    reserve_mb: self.config.reserve_mb.0,
                    total_mb: system.total_mb.0,
                });
            }

            let record = MemoryLeaseRecord {
                id: Uuid::new_v4().to_string(),
                kind: request.kind,
                owner_pid: std::process::id(),
                worker_pid: None,
                reserved_mb: request.reserved_mb.0,
                startup_slot: request.startup_slot,
                label: request.label,
                created_at_epoch_s: unix_epoch_s(),
            };
            let lease =
                HostMemoryLease::new(self.config.coordinator_path.clone(), record.id.clone());
            ledger.leases.push(record);
            Ok(lease)
        })
    }
}

fn with_locked_ledger<T>(
    path: &Path,
    action: impl FnOnce(&mut MemoryLedger) -> Result<T, HostMemoryError>,
) -> Result<T, HostMemoryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| HostMemoryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|source| HostMemoryError::Io {
            path: path.to_path_buf(),
            source,
        })?;

    file.lock_exclusive()
        .map_err(|source| HostMemoryError::Io {
            path: path.to_path_buf(),
            source,
        })?;

    let mut raw = String::new();
    file.read_to_string(&mut raw)
        .map_err(|source| HostMemoryError::Io {
            path: path.to_path_buf(),
            source,
        })?;

    let mut ledger = if raw.trim().is_empty() {
        MemoryLedger::default()
    } else {
        serde_json::from_str::<MemoryLedger>(&raw).map_err(|error| {
            HostMemoryError::CorruptLedger {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?
    };

    prune_stale_leases(&mut ledger);

    let result = action(&mut ledger);

    file.set_len(0).map_err(|source| HostMemoryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| HostMemoryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::to_writer_pretty(&mut file, &ledger).map_err(|error| {
        HostMemoryError::CorruptLedger {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    file.write_all(b"\n")
        .map_err(|source| HostMemoryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| HostMemoryError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    result
}

/// Pre-spawn intent leases (those with `worker_pid: None` whose owner
/// is the live daemon) must auto-expire if the spawn never
/// materialises. Without this, a failed-spawn or daemon-side bookkeeping
/// glitch produces a phantom hold that consumes budget indefinitely.
///
/// 30 minutes is generous: Stanza model load is typically 1–3 minutes
/// on cold start, and the daemon's own ready-timeout is configured at
/// `ready_timeout_s` (default 600s = 10 min) plus retry headroom. A
/// pre-spawn intent older than this window is by construction stuck.
///
/// Operational evidence (2026-05-01 afternoon wedge): 3 admitted jobs
/// each held ~76 GB JobExecution leases with `worker_pid: None`; no
/// workers ever spawned for them; the 228 GB sat as phantom holds for
/// hours. Without a deadline, every "failed admission cycle" leaks
/// budget.
const PRE_SPAWN_INTENT_DEADLINE_S: u64 = 1_800;

fn prune_stale_leases(ledger: &mut MemoryLedger) {
    let now = unix_epoch_s();
    ledger.leases.retain(|lease| {
        // Bug 2 fix (Contract B): when a lease names a specific worker
        // subprocess, its survival is bound to that worker, NOT to
        // the daemon. Workers run in their own process group
        // (`setpgid(0, 0)`) and can die independently — OOM, crash,
        // external kill — without the daemon noticing. Tagging the
        // lease with the worker's PID and checking THAT PID's
        // liveness is what closes the ghost-slot accounting drift
        // that wedged ming on 2026-05-01.
        if let Some(worker_pid) = lease.worker_pid {
            return process_is_alive(worker_pid);
        }

        // No worker_pid: this is a pre-spawn intent or daemon-side
        // bookkeeping lease. First check the owner's liveness — a dead
        // daemon's leases are reclaimed at first ledger access (Contract
        // F, restart hygiene).
        if !process_is_alive(lease.owner_pid) {
            return false;
        }

        // Owner is alive but no worker_pid is bound. Apply the
        // pre-spawn intent deadline (Contract E): if the lease is
        // older than the deadline window, the spawn it was reserving
        // for didn't materialise — reclaim the budget. A live owner
        // that legitimately holds a long-lived bookkeeping lease can
        // refresh `created_at_epoch_s` on its own cadence; this prune
        // protects against the daemon-side leak path, not against
        // healthy long-lived state.
        let age_s = now.saturating_sub(lease.created_at_epoch_s);
        age_s < PRE_SPAWN_INTENT_DEADLINE_S
    });
}

fn process_is_alive(pid: u32) -> bool {
    let mut system = System::new();
    let pid = Pid::from_u32(pid);
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    system.process(pid).is_some()
}

/// TTL the cached `SystemMemorySnapshot` is good for. Admission gates
/// and the eviction pre-pass both poll memory; with a 1 s TTL the
/// kernel-side `/proc/meminfo` (Linux) / `host_statistics64` (macOS)
/// is read at most once per second across all callers, regardless
/// of how many spawn admissions or health-check ticks land in
/// between.
const SNAPSHOT_TTL: Duration = Duration::from_millis(1000);

/// Process-global TTL-cached memory snapshot. Internal to this
/// module; callers go through [`detect_total_memory_mb`] /
/// [`detect_available_memory_mb`].
static SNAPSHOT_CACHE: LazyLock<Mutex<(SystemMemorySnapshot, Instant)>> = LazyLock::new(|| {
    let snap = refresh_system_memory_snapshot();
    Mutex::new((snap, Instant::now()))
});

fn refresh_system_memory_snapshot() -> SystemMemorySnapshot {
    let mut system = System::new();
    system.refresh_memory();
    SystemMemorySnapshot {
        total_mb: MemoryMb(system.total_memory() / (1024 * 1024)),
        available_mb: MemoryMb(system.available_memory() / (1024 * 1024)),
    }
}

fn system_memory_snapshot() -> SystemMemorySnapshot {
    // Recover from a poisoned mutex — the snapshot has no invariant
    // that a panicking holder could leave broken; it's just a
    // stale-cache pair.
    let mut state = SNAPSHOT_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.1.elapsed() >= SNAPSHOT_TTL {
        state.0 = refresh_system_memory_snapshot();
        state.1 = Instant::now();
    }
    state.0
}

/// Total physical RAM on this host, in MB.
///
/// Public delegator over `system_memory_snapshot()` so the
/// `host_facts::RealHostFactsSource` can populate `HostFacts::ram_total_mb`
/// without depending on this module's internal `SystemMemorySnapshot`
/// shape. Held in this module so all sysinfo polling for the host's
/// RAM lives in one place.
pub fn detect_total_memory_mb() -> MemoryMb {
    system_memory_snapshot().total_mb
}

/// Currently-available RAM on this host, in MB.
///
/// "Available" follows the sysinfo definition (`free + reclaimable`),
/// which on macOS undercounts vs Activity Monitor's "memory pressure"
/// — see `worker::memory_guard` for the live polling helper that
/// applies tier-aware reserves on top of this raw figure.
pub fn detect_available_memory_mb() -> MemoryMb {
    system_memory_snapshot().available_mb
}

fn pressure_level_for(available_mb: MemoryMb, reserve_mb: MemoryMb) -> HostMemoryPressureLevel {
    if available_mb.0 <= reserve_mb.0 {
        return HostMemoryPressureLevel::Critical;
    }
    let extra_headroom = available_mb.0.saturating_sub(reserve_mb.0);
    if extra_headroom <= 2_048 {
        HostMemoryPressureLevel::Constrained
    } else if extra_headroom <= 8_192 {
        HostMemoryPressureLevel::Guarded
    } else {
        HostMemoryPressureLevel::Healthy
    }
}

/// Cap the configured `reserve_mb` headroom at this fraction of the
/// host's available RAM so a static reserve sized for a 64 GB server
/// doesn't lock out admission on a 16 GB laptop.
const RESERVE_HEADROOM_FRACTION_DENOM: u64 = 4;

fn plan_job_reservation(
    requested_workers: usize,
    per_worker_budget_mb: u64,
    available_mb: u64,
    reserve_mb: u64,
    pending_reserved_mb: u64,
) -> Option<(usize, u64)> {
    let effective_reserve_mb = reserve_mb.min(available_mb / RESERVE_HEADROOM_FRACTION_DENOM);
    for workers in (1..=requested_workers).rev() {
        let requested_mb = per_worker_budget_mb.saturating_mul(workers as u64);
        let projected_available_mb = available_mb
            .saturating_sub(pending_reserved_mb)
            .saturating_sub(requested_mb);
        if projected_available_mb >= effective_reserve_mb {
            return Some((workers, requested_mb));
        }
    }
    None
}

fn retryable_error(error: &HostMemoryError) -> bool {
    match error {
        HostMemoryError::CapacityRejected {
            total_mb,
            requested_mb,
            reserve_mb,
            ..
        } => capacity_rejection_is_retryable(*total_mb, *requested_mb, *reserve_mb),
        HostMemoryError::MlTestLockBusy { .. } | HostMemoryError::StartupSlotsBusy { .. } => true,
        HostMemoryError::Io { .. }
        | HostMemoryError::CorruptLedger { .. }
        | HostMemoryError::TimedOut { .. } => false,
    }
}

fn capacity_rejection_is_retryable(total_mb: u64, requested_mb: u64, reserve_mb: u64) -> bool {
    requested_mb.saturating_add(reserve_mb) <= total_mb
}

fn unix_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn host_ledger_suffix() -> String {
    let raw = std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("default"));
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        HostMemoryCoordinator, HostMemoryError, HostMemoryLease, HostMemoryPressureLevel,
        HostMemoryRuntimeConfig, MemoryLeaseKind, MemoryLeaseRecord, MemoryLedger,
        plan_job_reservation, pressure_level_for, retryable_error, unix_epoch_s,
        with_locked_ledger,
    };
    use crate::api::MemoryMb;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Build a runtime config pointing at a fresh tmp coordinator file.
    /// Returns the TempDir so the caller can keep it alive for the test.
    fn test_runtime_config() -> (TempDir, HostMemoryRuntimeConfig) {
        let dir = TempDir::new().expect("tmp dir for host-memory test");
        let coordinator_path: PathBuf = dir.path().join("ledger.json");
        let config = HostMemoryRuntimeConfig::from_sources(coordinator_path, MemoryMb(8_000), 4);
        (dir, config)
    }

    /// Test helper: inject a synthetic lease into the ledger so we can
    /// drive prune_stale_leases without spawning real workers.
    fn inject_lease(
        config: &HostMemoryRuntimeConfig,
        kind: MemoryLeaseKind,
        owner_pid: u32,
        worker_pid: Option<u32>,
        reserved_mb: u64,
        label: &str,
    ) {
        with_locked_ledger(&config.coordinator_path, |ledger: &mut MemoryLedger| {
            ledger.leases.push(MemoryLeaseRecord {
                id: Uuid::new_v4().to_string(),
                kind,
                owner_pid,
                worker_pid,
                reserved_mb,
                startup_slot: matches!(kind, MemoryLeaseKind::WorkerStartup),
                label: label.to_owned(),
                created_at_epoch_s: unix_epoch_s(),
            });
            Ok(())
        })
        .expect("inject lease");
    }

    /// Bug 2 (ghost slots) regression: when a worker subprocess dies
    /// without the daemon explicitly releasing its lease, the ledger
    /// retains a phantom reservation that blocks future jobs from
    /// fitting. Pruning MUST detect a dead worker_pid and drop the
    /// lease so reserved memory accounting stays in sync with reality.
    ///
    /// Failure mode (pre-fix): the lease's `owner_pid` is the DAEMON's
    /// PID. The daemon stays alive, so `process_is_alive(owner_pid)`
    /// returns true and the lease is never pruned. The reservation
    /// accumulates as ghost slots — exactly the 2026-05-01 wedge.
    #[test]
    fn lease_for_dead_worker_pid_is_pruned_on_next_ledger_access() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());

        // PID 4_000_000 is a known-dead PID per the reaper test convention.
        let dead_worker_pid: u32 = 4_000_000;
        let daemon_pid = std::process::id();

        // Inject a phantom lease tagged with the dead worker's PID.
        inject_lease(
            &config,
            MemoryLeaseKind::JobExecution,
            daemon_pid,
            Some(dead_worker_pid),
            12_000,
            "test-ghost-slot",
        );

        // Sanity-check: the lease was inserted.
        let pre_prune = coord
            .snapshot()
            .expect("snapshot before second prune cycle");
        // First snapshot call already runs prune_stale_leases once.
        // After the fix, the lease should already be gone here:
        assert_eq!(
            pre_prune.active_reserved_mb,
            MemoryMb(0),
            "lease whose worker_pid is dead must be pruned, but ledger still reports {} MB reserved",
            pre_prune.active_reserved_mb.0
        );
    }

    /// End-to-end regression: the daemon-side flow is
    ///   1. acquire a lease (no worker_pid yet — daemon's PID held)
    ///   2. spawn the worker subprocess (now `Child::id()` is known)
    ///   3. tag the lease with the worker's PID via `set_worker_pid`
    ///   4. when the worker dies later, prune drops the lease.
    ///
    /// This test exercises the API surface that the worker-spawn code
    /// must call. Without `set_worker_pid` plumbed at the spawn site,
    /// the field stays `None` and Bug 2 is unfixed in deployed binaries.
    #[test]
    fn set_worker_pid_makes_subsequent_prune_target_the_worker() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());

        // 1. Acquire a synthetic lease the way the daemon would —
        //    owner_pid = daemon (this process), worker_pid = None.
        let lease_id = Uuid::new_v4().to_string();
        with_locked_ledger(&config.coordinator_path, |ledger| {
            ledger.leases.push(MemoryLeaseRecord {
                id: lease_id.clone(),
                kind: MemoryLeaseKind::WorkerStartup,
                owner_pid: std::process::id(),
                worker_pid: None,
                reserved_mb: 12_000,
                startup_slot: true,
                label: "test-startup-lease".to_owned(),
                created_at_epoch_s: unix_epoch_s(),
            });
            Ok(())
        })
        .expect("inject startup lease");

        // 2. Build the public-API HostMemoryLease handle for that
        //    record (mirrors what acquire_worker_startup_lease returns).
        let lease = HostMemoryLease::new(config.coordinator_path.clone(), lease_id.clone());

        // 3. Tag the lease with a known-dead worker PID — same
        //    convention the reaper test uses.
        let dead_worker_pid: u32 = 4_000_000;
        lease
            .set_worker_pid(dead_worker_pid)
            .expect("set_worker_pid should rewrite the ledger");

        // Don't drop the lease yet — Drop calls release_internal which
        // would remove the record outright and obscure whether prune
        // did the job.
        std::mem::forget(lease);

        // 4. Trigger a prune cycle by reading the ledger via snapshot.
        let snap = coord.snapshot().expect("snapshot");
        assert_eq!(
            snap.active_reserved_mb,
            MemoryMb(0),
            "after set_worker_pid pointed at a dead PID, prune must drop the lease"
        );
    }

    // ============================================================
    // Memory-accounting contract tests (2026-05-01 Feynman-discipline
    // pass). These tests are written from FIRST PRINCIPLES about what
    // a memory-reservation subsystem should do, not from inspection of
    // the implementation. Tests that fail are evidence that either the
    // contract is wrong (correct it) or the system violates it (bug).
    // ============================================================

    /// Contract A — Conservation:
    /// Σ(reserved across leases) must never exceed (total − headroom).
    /// The coordinator should refuse a request that would push the
    /// total over capacity, not allocate beyond capacity.
    ///
    /// Test: simulate a finite-capacity host. Acquire reservations
    /// that fit. The next request that would exceed capacity must be
    /// rejected with CapacityRejected. The ledger sum must never
    /// transiently exceed (total − headroom).
    #[test]
    fn contract_a_capacity_rejection_when_sum_would_exceed_budget() {
        // For this test we exercise the planner directly because the
        // system-memory snapshot reads real OS memory and we can't
        // fake total RAM in a unit test without a deeper plumb.
        //
        // The contract statement is enforced by `plan_job_reservation`:
        // when the worst-case requested allocation would push beyond
        // (available - reserve_mb), it must return None (rejection).
        //
        // For a worst-case-rejecting-correctly test:
        //   available = 32 GB
        //   pending = 20 GB already reserved
        //   reserve_mb = 8 GB headroom
        //   per_worker = 8 GB; requesting 1 worker
        //
        // Math:  remaining = 32 - 20 - 8 = 4 GB; 1 worker needs 8 GB.
        // Expected: rejection (None).
        let result = plan_job_reservation(1, 8_000, 32_000, 8_000, 20_000);
        assert!(
            result.is_none(),
            "Contract A violated: planner allowed 1 worker × 8 GB when only 4 GB free above headroom (got {result:?})"
        );

        // Allowed case: reduce pending to leave room for 1 worker.
        let allowed = plan_job_reservation(1, 8_000, 32_000, 8_000, 16_000);
        assert!(
            allowed.is_some(),
            "Contract A: planner refused a request that should fit (32 - 16 - 8 - 8 = 0, equal to threshold)"
        );
    }

    /// Contract B — Liveness:
    /// A reservation tagged with a process PID must be reclaimed when
    /// that process dies. Already covered by the prune tests above
    /// (`lease_for_dead_worker_pid_is_pruned_on_next_ledger_access`),
    /// reasserted here as the named contract.
    #[test]
    fn contract_b_lease_reclaimed_when_bound_process_dies() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());
        let dead_pid: u32 = 4_000_000;
        inject_lease(
            &config,
            MemoryLeaseKind::WorkerStartup,
            std::process::id(),
            Some(dead_pid),
            8_000,
            "contract-b-test",
        );
        let snap = coord.snapshot().expect("snapshot");
        assert_eq!(
            snap.active_reserved_mb,
            MemoryMb(0),
            "Contract B violated: lease bound to dead PID {dead_pid} not reclaimed"
        );
    }

    /// Contract D — No double-counting:
    /// A single live worker subprocess must be represented by exactly
    /// one reservation, not multiple stacking ones. If a job has a
    /// JobExecution lease covering its workers' budget, individual
    /// WorkerStartup leases for those same workers must NOT add to
    /// the daemon-side total reserved figure — they should be
    /// sub-allocations, not additions.
    ///
    /// Operational evidence (2026-05-01): with 3 admitted jobs each
    /// reserving ~76 GB at JobExecution level, the daemon ALSO
    /// counts WorkerStartup leases on top, producing the 228 GB ghost
    /// even though only 3 jobs are active. This test pins what the
    /// contract should be — and SHOULD FAIL TODAY.
    #[test]
    fn contract_d_worker_startup_does_not_double_count_with_job_execution() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());

        // Inject a JobExecution lease that already reserves 96 GB
        // (the worst-case 8-worker × 12 GB plan).
        inject_lease(
            &config,
            MemoryLeaseKind::JobExecution,
            std::process::id(),
            None,
            96_000,
            "contract-d-job-exec",
        );
        // Now acquire a WorkerStartup lease for one of those workers.
        // Under correct accounting, this should NOT add to the
        // daemon-side total because the JobExecution reservation
        // already covers it.
        inject_lease(
            &config,
            MemoryLeaseKind::WorkerStartup,
            std::process::id(),
            Some(std::process::id()),
            12_000,
            "contract-d-worker-startup",
        );

        let snap = coord.snapshot().expect("snapshot");
        assert_eq!(
            snap.active_reserved_mb,
            MemoryMb(96_000),
            "Contract D violated: WorkerStartup double-counts on top of JobExecution; total reserved is {} MB, expected 96000 MB",
            snap.active_reserved_mb.0
        );
    }

    /// Contract E — Pre-spawn intent must not phantom-hold capacity:
    /// A reservation made for a worker that has not yet spawned (i.e.,
    /// `worker_pid: None`, owner_pid = daemon) must either:
    ///   (a) be tied to a deadline, automatically reclaimed if the
    ///       spawn doesn't materialize within the window; OR
    ///   (b) be tagged with the worker's PID promptly after spawn,
    ///       so liveness binding kicks in.
    ///
    /// If neither holds, a failed spawn or runaway daemon code path
    /// produces a phantom hold that consumes budget forever.
    ///
    /// Test: simulate the "spawn never materialises" case. Acquire a
    /// startup-style reservation and never tag it with a worker PID.
    /// Wait the prune cycle. The reservation must be reclaimed.
    ///
    /// Expectation: SHOULD FAIL TODAY. The pre-spawn intent has no
    /// deadline mechanism in the current code; daemon-PID-bound
    /// reservations survive indefinitely.
    #[test]
    fn contract_e_pre_spawn_reservation_without_worker_pid_must_have_deadline() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());

        // Inject a "pending spawn" reservation: owner = daemon (alive),
        // worker_pid = None (no worker exists yet), age beyond any
        // reasonable spawn deadline.
        inject_lease(
            &config,
            MemoryLeaseKind::WorkerStartup,
            std::process::id(),
            None,
            12_000,
            "contract-e-pending-spawn-aged",
        );

        // Backdate the lease's created_at so it's clearly past any
        // reasonable startup window.
        with_locked_ledger(&config.coordinator_path, |ledger| {
            for lease in ledger.leases.iter_mut() {
                lease.created_at_epoch_s = 1;
            }
            Ok(())
        })
        .unwrap();

        let snap = coord.snapshot().expect("snapshot");
        assert_eq!(
            snap.active_reserved_mb,
            MemoryMb(0),
            "Contract E violated: a pre-spawn reservation that never materialised into a worker is held indefinitely (got {} MB still reserved)",
            snap.active_reserved_mb.0
        );
    }

    /// Contract C — A 64 GB host with clean state must admit at least
    /// a 1-worker morphotag job. Per-worker budget for morphotag is
    /// 12 GB (8000 base × 1.5 loading_overhead, process mode); reserve
    /// headroom is 8 GB. With 64 GB available and 0 pending, the
    /// projected leftover after granting 1 worker is 64 − 0 − 12 = 52
    /// GB ≥ 8 GB reserve. The planner MUST grant at least 1 worker.
    ///
    /// This is the "tragicomic" case from 2026-05-01: 64 GB hosts
    /// (bilbo/sue/lilly/vaishnavi) reportedly couldn't run morphotag
    /// with default config, even though "they sure as hell can."
    /// This test pins what the planner is supposed to do.
    #[test]
    fn contract_c_64gb_host_with_clean_state_admits_1_worker_morphotag() {
        // morphotag per-worker = 8000 × 1.5 = 12000 MB (process mode)
        let result = plan_job_reservation(
            1,      // requested_workers
            12_000, // per_worker_budget
            64_000, // available_mb (clean 64 GB host, no other load)
            8_000,  // reserve_mb (host headroom)
            0,      // pending_reserved_mb
        );
        assert!(
            result.is_some(),
            "Contract C violated: 64 GB host with clean state cannot admit 1-worker morphotag (per-worker=12 GB, reserve=8 GB). Math: 64 - 0 - 12 = 52 ≥ 8."
        );
        let (granted, reserved_mb) = result.unwrap();
        assert_eq!(granted, 1);
        assert_eq!(reserved_mb, 12_000);
    }

    /// Contract C (laptop variant) — A 16 GB laptop must admit a
    /// 1-worker morphotag job per the design intent stated in
    /// `runtime.rs:218`: "This allows batchalign3 to run on 16 GB
    /// laptops through 256 GB servers without manual tuning."
    ///
    /// Math under current constants: 16 - 0 - 12 = 4 < 8 reserve → reject.
    /// Expected: SHOULD FAIL TODAY. The 8 GB reserve is more than
    /// half a 16 GB laptop's RAM, so the planner can't fit even a
    /// single worker. The fix is tier-derived reserve sizing (small
    /// hosts get a smaller reserve).
    #[test]
    fn contract_c_16gb_laptop_with_clean_state_admits_1_worker_morphotag() {
        let result = plan_job_reservation(1, 12_000, 16_000, 8_000, 0);
        assert!(
            result.is_some(),
            "Contract C violated: 16 GB laptop with clean state cannot admit 1-worker morphotag. Math: 16 - 0 - 12 = 4, reserve=8. Either the per-worker budget is too high or the reserve is too high for small hosts. Design intent (runtime.rs:218): 'batchalign3 runs on 16 GB laptops through 256 GB servers without manual tuning' — that's not happening with current constants."
        );
    }

    /// Contract F — Atomicity across daemon crash:
    /// On daemon restart, leases owned by the (now-dead) prior daemon
    /// PID must be reclaimed before any new accounting decision.
    ///
    /// Test: write a ledger file in which all leases are owned by a
    /// dead PID and have `worker_pid: None`. Build a new coordinator
    /// pointing at that ledger. The first snapshot must show zero
    /// reservations.
    ///
    /// Expectation: SHOULD PASS TODAY (the existing
    /// `prune_stale_leases` does this for `worker_pid: None` leases
    /// via the owner_pid liveness check). This contract test is a
    /// regression guard.
    #[test]
    fn contract_f_dead_daemon_pid_leases_reclaimed_at_first_access() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());

        let dead_daemon_pid: u32 = 4_000_001;
        inject_lease(
            &config,
            MemoryLeaseKind::JobExecution,
            dead_daemon_pid,
            None,
            96_000,
            "contract-f-prior-daemon",
        );

        let snap = coord.snapshot().expect("snapshot");
        assert_eq!(
            snap.active_reserved_mb,
            MemoryMb(0),
            "Contract F violated: lease owned by dead daemon PID {dead_daemon_pid} survived first ledger access ({} MB still reserved)",
            snap.active_reserved_mb.0
        );
    }

    /// Counterpart: a lease whose worker_pid IS alive must be retained.
    /// This guards against an over-eager prune that would drop healthy
    /// reservations and double-book memory.
    #[test]
    fn lease_for_live_worker_pid_is_retained() {
        let (_dir, config) = test_runtime_config();
        let coord = HostMemoryCoordinator::new(config.clone());

        // Use the test process's own PID — guaranteed alive.
        let live_pid = std::process::id();

        inject_lease(
            &config,
            MemoryLeaseKind::JobExecution,
            live_pid,
            Some(live_pid),
            12_000,
            "test-live-slot",
        );

        let snap = coord.snapshot().expect("snapshot");
        assert_eq!(
            snap.active_reserved_mb,
            MemoryMb(12_000),
            "lease for live worker_pid must be retained"
        );
    }

    #[test]
    fn pressure_levels_follow_available_headroom() {
        assert_eq!(
            pressure_level_for(MemoryMb(64_000), MemoryMb(8_192)),
            HostMemoryPressureLevel::Healthy
        );
        assert_eq!(
            pressure_level_for(MemoryMb(12_000), MemoryMb(8_192)),
            HostMemoryPressureLevel::Guarded
        );
        assert_eq!(
            pressure_level_for(MemoryMb(9_000), MemoryMb(8_192)),
            HostMemoryPressureLevel::Constrained
        );
        assert_eq!(
            pressure_level_for(MemoryMb(8_192), MemoryMb(8_192)),
            HostMemoryPressureLevel::Critical
        );
    }

    /// On 32 GB available the 8192 MB reserve caps at 8000 MB; the
    /// planner clamps 8 requested workers down to 4.
    #[test]
    fn job_planner_reduces_worker_count_to_fit_headroom() {
        let planned =
            plan_job_reservation(8, 6_000, 32_000, 8_192, 0).expect("some worker count should fit");
        assert_eq!(planned, (4, 24_000));
    }

    /// Pending reservations are subtracted before the headroom check.
    #[test]
    fn job_planner_accounts_for_pending_reservations() {
        let planned = plan_job_reservation(4, 4_000, 24_000, 8_192, 6_000)
            .expect("some worker count should fit");
        assert_eq!(planned, (3, 12_000));
    }

    /// On a host where even one worker would overflow the headroom
    /// after pending demand, the planner rejects.
    #[test]
    fn job_planner_rejects_when_even_one_worker_breaks_reserve() {
        assert!(plan_job_reservation(2, 8_000, 6_000, 8_192, 1_000).is_none());
    }

    #[test]
    fn impossible_capacity_rejection_is_not_retryable() {
        let error = HostMemoryError::CapacityRejected {
            label: String::from("impossible"),
            available_mb: 32_000,
            pending_reserved_mb: 0,
            requested_mb: 80_000,
            reserve_mb: 8_192,
            total_mb: 64_000,
        };
        assert!(
            !retryable_error(&error),
            "impossible reservations should fail immediately"
        );
    }

    #[test]
    fn capacity_rejection_stays_retryable_when_it_can_fit_later() {
        let error = HostMemoryError::CapacityRejected {
            label: String::from("contended"),
            available_mb: 6_000,
            pending_reserved_mb: 20_000,
            requested_mb: 16_000,
            reserve_mb: 8_192,
            total_mb: 64_000,
        };
        assert!(
            retryable_error(&error),
            "requests that could fit later should keep retry semantics"
        );
    }
}
