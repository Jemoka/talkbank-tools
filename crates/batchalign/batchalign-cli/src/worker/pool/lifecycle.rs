//! Background tasks: health checking, idle timeout, worker spawning helpers.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::api::{NumSpeakers, WorkerLanguage};
use tracing::{error, info, warn};

use crate::worker::WorkerTarget;
use crate::worker::error::WorkerError;
use crate::worker::handle::{WorkerConfig, WorkerHandle};

use super::cpu_gate;
use super::idle_eviction;
use super::memory_gate;
use super::permit::{PermitRejected, SpawnPermitGuard};
use super::rss_observer;
use super::{GroupsMap, WorkerGroup, WorkerKey, WorkerPool};

/// Reason a `try_claim_spawn_slot` call was rejected. Distinguishes
/// the admission stages so callers can treat them differently
/// (per-key rejection means "wait for someone to return a worker for
/// THIS key"; global rejection means "wait for ANY worker anywhere
/// to exit"; CPU saturation and memory pressure mean "wait for the
/// host to become less loaded — no permit wait will help"). The
/// dispatch slow path uses the variant to choose between
/// `group.available.acquire()`, `pool.spawn_permits.acquire()`, and
/// a time-based retry.
#[derive(Debug)]
pub(super) enum AdmissionRejection {
    /// Global permit pool exhausted. The acquisition increments
    /// `permit_rejections_total`.
    GlobalCap,
    /// Per-key cap (`max_workers_per_key.get(group.profile)`)
    /// reached. The CAS loop returns this without holding the
    /// global permit (the guard auto-drops). Increments
    /// `spawn_rejections_total` — currently the per-key counter.
    PerKeyCap,
    /// 1-minute CPU load average is at or above the host's logical
    /// CPU count. Adding another worker would oversubscribe; the
    /// admission seam refuses until live load drops. Increments
    /// `cpu_saturation_rejections_total`. Unlike `GlobalCap` and
    /// `PerKeyCap`, this rejection isn't unblocked by a worker
    /// returning — only by the host itself becoming less busy.
    CpuSaturated,
    /// Host's currently-available memory is at or below the
    /// hardcoded minimum-free floor (`memory_gate::MIN_FREE_MEMORY_MB`).
    /// Adding another worker risks an OOM-class outcome that no
    /// permit accounting can prevent; the admission seam refuses
    /// until live free memory rises. Increments
    /// `memory_constrained_rejections_total`. Like `CpuSaturated`,
    /// this rejection isn't unblocked by a worker returning — only
    /// by the host itself freeing memory (other processes exiting,
    /// caches reclaiming, etc.).
    MemoryConstrained,
}

impl From<PermitRejected> for AdmissionRejection {
    fn from(_: PermitRejected) -> Self {
        AdmissionRejection::GlobalCap
    }
}

/// Logarithmic gate for the "global worker cap reached" WARN: emit
/// only when the rejection count is a power of two (1, 2, 4, 8, 16, …).
/// Sustained saturation produces O(log N) WARN events instead of N.
/// The monotonic `spawn_rejections_total` counter on the pool is the
/// authoritative observability surface; the log is a coarse signal.
pub(super) fn should_log_saturation(rejection_count: u64) -> bool {
    rejection_count.is_power_of_two()
}

impl WorkerPool {
    /// Start background tasks for health checking and pressure-driven
    /// eviction.
    ///
    /// Returns a `JoinHandle` that completes when the pool is shut down.
    pub fn start_background_tasks(&self) -> tokio::task::JoinHandle<()> {
        let groups = self.groups.clone();
        let cancel = self.cancel.clone();
        let health_interval = Duration::from_secs(self.config.health_check_interval_s);
        let pool_config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(health_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("Worker pool background tasks cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        run_health_check(&groups, &pool_config).await;
                        // Reap orphaned workers left behind by previous server
                        // crashes (SIGKILL, OOM). This is cheap (reads a small
                        // directory) and catches orphans that would otherwise
                        // hold 2-15 GB each until the next server restart.
                        let reaped = super::reaper::reap_orphaned_workers();
                        if reaped > 0 {
                            info!(reaped, "Periodic orphan reaper cleaned up workers");
                        }
                    }
                }
            }
        })
    }

    /// Build a `WorkerConfig` for the given worker profile and worker language.
    pub(super) fn worker_config(
        &self,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
    ) -> WorkerConfig {
        WorkerConfig {
            python_path: self.config.python_path.clone(),
            profile: target.profile_kind(),
            task: target.task(),
            lang: lang.clone(),
            num_speakers: NumSpeakers(1),
            engine_overrides: engine_overrides.to_owned(),
            test_echo: self.config.test_echo,
            ready_timeout_s: self.config.ready_timeout_s,
            verbose: self.config.verbose,
            runtime: self.config.runtime.clone(),
            audio_task_timeout_s: self.config.audio_task_timeout_s,
            analysis_task_timeout_s: self.config.analysis_task_timeout_s,
            test_delay_ms: self.config.test_delay_ms,
        }
    }

    /// Get or create the `WorkerGroup` for a key.
    pub(super) fn get_or_create_group(
        &self,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
    ) -> Arc<WorkerGroup> {
        let key: super::WorkerKey = (*target, lang.clone(), engine_overrides.to_owned());
        let profile = target.profile_kind();
        let mut groups = super::lock_recovered(&self.groups);
        groups
            .entry(key)
            .or_insert_with(|| {
                Arc::new(WorkerGroup::new(
                    self.worker_returned.clone(),
                    self.spawn_permits.clone(),
                    profile,
                ))
            })
            .clone()
    }

    /// Try to atomically claim a spawn slot in a group via compare_exchange.
    ///
    /// Checks two limits:
    /// 1. Per-key cap: `max_workers_per_key.<group.profile>` (prevents
    ///    one key from hogging within its profile).
    /// 2. Global cap: `max_total_workers` (prevents aggregate OOM).
    ///
    /// Returns `Ok(claimed_total)` if a slot was claimed, `Err(current)` if
    /// at capacity.
    pub(super) fn try_claim_spawn_slot(
        &self,
        group: &WorkerGroup,
    ) -> Result<(usize, SpawnPermitGuard), AdmissionRejection> {
        let max = self.config.max_workers_per_key.get(group.profile);

        // Pressure gates (CPU loadavg, memory headroom) implement
        // BACK-PRESSURE — they slow new spawns when existing workers
        // are pressuring the host. Cold-start (no workers in this
        // group) has nothing to back-pressure against; refusing here
        // leaves the pool dead-on-arrival with no recovery path.
        // Both gates honor the same cold-start bypass via their
        // `_with_state` variants. Capacity gates (Layer 1 permit,
        // Layer 2 per-key cap) below run regardless: those enforce
        // budget, not pressure.
        //
        // Safety against actual at-spawn OOM is one layer down:
        // `memory_guard` per-worker RSS observation + the OS OOM
        // killer. The pressure gates are an optimization for healthy
        // hosts, not load-bearing for correctness.
        let pool_state = if group.is_empty() {
            memory_gate::PoolGateState::ColdStart
        } else {
            memory_gate::PoolGateState::Warm
        };

        // Layer 0: live CPU-loadavg gate. Refusing here before any
        // permit work means a saturated host doesn't churn the
        // global-permit semaphore. Threshold is the host's logical
        // CPU count, polled fresh each call. Tests exercising the
        // permit / per-key-cap / metrics paths set the override to a
        // value far above any realistic load so the gate does not
        // reject before reaching the test logic; the dedicated
        // cpu_gate unit tests cover both branches.
        let cpu_threshold = self
            .config
            .cpu_gate_threshold_override
            .unwrap_or_else(cpu_gate::host_cpu_count_as_threshold);
        if let Err(saturated) = cpu_gate::check_cpu_saturation_with_state(pool_state, cpu_threshold)
        {
            let count = self
                .cpu_saturation_rejections_total
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            if should_log_saturation(count) {
                warn!(
                    loadavg_1m = saturated.loadavg_1m,
                    threshold = saturated.threshold,
                    rejection_count = count,
                    "Host CPU saturated, rejecting worker spawn"
                );
            }
            return Err(AdmissionRejection::CpuSaturated);
        }

        // Layer 0.5: live available-memory gate, with forward-looking
        // projection (Mode A) and observed-peer RSS substitution
        // (Mode B). Sibling to the CPU gate. Refuses spawns at
        // admission rather than reactively inside
        // `WorkerHandle::spawn`'s memory_guard, so a memory-constrained
        // host doesn't burn through the global-permit semaphore on
        // doomed spawns.
        //
        // The predicate is `available_mb - estimate_mb > floor`.
        // The floor is the hardcoded `memory_gate::MIN_FREE_MEMORY_MB`
        // (= 2 GB OS-protection headroom). The estimate is the new
        // worker's projected memory cost; we prefer the observed
        // average RSS of same-profile idle peers (Mode B), falling
        // back to the static `startup_reservation_mb_for_tier` value
        // (Mode A) when no peers are available. The fallback ensures
        // that pre-warmup admission decisions match Mode A behavior
        // exactly — Mode B is strictly an improvement, never worse.
        //
        // No env var, no PoolConfig field, no operator override.
        // Tier-scaled floor — the 2048 MB hardcode the rearch shipped
        // was right for Medium workstations and wrong on every other
        // tier. See `host_min_free_mb_threshold_for_tier`.
        let mem_threshold =
            memory_gate::host_min_free_mb_threshold_for_tier(&self.config.runtime.memory_tier);
        let reservation_mb = group
            .profile
            .startup_reservation_mb_for_tier(&self.config.runtime.memory_tier)
            .0;
        let (estimate_mb, estimate_source) =
            match rss_observer::observed_avg_rss_mb_for_profile(&self.groups, group.profile) {
                Some(observed) => (observed, rss_observer::EstimateSource::ObservedAvgIdle),
                None => (reservation_mb, rss_observer::EstimateSource::Reservation),
            };
        if let Err(constrained) =
            memory_gate::check_memory_saturation_with_state(pool_state, mem_threshold, estimate_mb)
        {
            let count = self
                .memory_constrained_rejections_total
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            if should_log_saturation(count) {
                let estimate_source_label = match estimate_source {
                    rss_observer::EstimateSource::Reservation => "reservation",
                    rss_observer::EstimateSource::ObservedAvgIdle => "observed_avg_idle",
                };
                warn!(
                    available_mb = constrained.available_mb,
                    estimate_mb = constrained.reservation_mb,
                    estimate_source = estimate_source_label,
                    threshold_mb = constrained.threshold_mb,
                    projected_after_spawn_mb = constrained
                        .available_mb
                        .saturating_sub(constrained.reservation_mb),
                    rejection_count = count,
                    "Projected post-spawn memory below floor, rejecting worker spawn"
                );
            }
            return Err(AdmissionRejection::MemoryConstrained);
        }

        // Layer 1: acquire a global-cap permit. Failure here means
        // every worker slot allowed by `max_total_workers` is in use
        // (or speculatively claimed by another concurrent admission)
        // and the caller must wait on `spawn_permits.acquire()` rather
        // than re-probe.
        let guard = SpawnPermitGuard::try_acquire(&self.spawn_permits).map_err(|e| {
            let count = self.permit_rejections_total.fetch_add(1, Ordering::Relaxed) + 1;
            if should_log_saturation(count) {
                warn!(
                    permits_available = self.spawn_permits.available_permits(),
                    max_total = self.config.effective_max_total_workers(),
                    rejection_count = count,
                    "Global permit pool exhausted, rejecting spawn"
                );
            }
            AdmissionRejection::from(e)
        })?;

        // Layer 2: per-key cap CAS. The permit guard is held across
        // the loop; on per-key failure it drops here and refunds the
        // permit so other groups can use it. The CAS retry path
        // re-uses the same guard — we already paid for the global
        // slot, just need to win the per-key race.
        loop {
            let current = group.total.load(Ordering::Relaxed);
            if current >= max {
                // Per-key cap saturated. Bump the per-key counter;
                // the guard drops at function exit, refunding the
                // global permit so other groups can use it.
                self.spawn_rejections_total.fetch_add(1, Ordering::Relaxed);
                return Err(AdmissionRejection::PerKeyCap);
            }

            match group.total.compare_exchange(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok((current + 1, guard)),
                Err(_) => continue, // Lost the CAS race; retry without releasing the permit.
            }
        }
    }

    /// Spawn a worker into a group, using `try_claim_spawn_slot` for the
    /// atomic slot reservation.
    ///
    /// On success, the worker is pushed into the idle queue with a permit.
    /// On spawn failure, the slot is released.
    pub(super) async fn try_spawn_into_group(
        &self,
        group: &Arc<WorkerGroup>,
        target: &WorkerTarget,
        lang: &WorkerLanguage,
        engine_overrides: &str,
    ) -> Result<bool, WorkerError> {
        let guard = match self.try_claim_spawn_slot(group) {
            Ok((_, g)) => g,
            Err(_) => return Ok(false), // At capacity
        };

        let _bootstrap_guard = group.bootstrap.lock().await;

        // Slot claimed -- now spawn. If spawn fails, release the slot.
        // The permit guard stays in scope across the spawn await: on
        // Ok we hand the permit's lifetime over to the worker (via
        // `guard.forget()`); on Err the guard drops at function exit
        // and refunds the permit alongside the per-key fetch_sub.
        match WorkerHandle::spawn(self.worker_config(target, lang, engine_overrides)).await {
            Ok(mut handle) => {
                // Lazily detect capabilities from the first spawned worker.
                // This is a single IPC round-trip on an already-running worker.
                if self.lazy_capabilities.get().is_none()
                    && let Err(e) = self.detect_capabilities_from_worker(&mut handle).await
                {
                    tracing::warn!(error = %e, "Failed to detect capabilities from first worker (continuing)");
                }
                // Don't use a separate push_spawned (which would double-increment
                // total). We already incremented via compare_exchange.
                super::lock_recovered(&group.idle).push_back(handle);
                group.available.add_permits(1);
                // Worker is officially counted; transfer permit
                // ownership to the worker's lifetime. The matching
                // release happens at the worker's exit point
                // (eviction, reaper, take(), shutdown drain).
                guard.forget();
                Ok(true)
            }
            Err(e) => {
                group.total.fetch_sub(1, Ordering::Relaxed);
                // guard drops here at function exit, refunding the permit.
                if matches!(e, WorkerError::MemoryGuard(_)) {
                    self.memory_gate_rejections_total
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e)
            }
        }
    }
}

/// Run a single round of pressure-driven eviction + health checks.
///
/// Only examines idle workers (checked-out workers are in use; errors
/// during dispatch are handled by the caller). Eviction is fully
/// pressure-driven via [`pressure_evict_idle_workers_if_needed`];
/// dead workers are removed from the idle queue by the liveness
/// loop and `total` is decremented.
pub(super) async fn run_health_check(groups_ref: &GroupsMap, pool_config: &super::PoolConfig) {
    pressure_evict_idle_workers_if_needed(groups_ref).await;

    // Snapshot group Arcs so we don't hold the groups lock across awaits.
    let group_snapshot: Vec<(WorkerKey, Arc<WorkerGroup>)> = {
        let groups = super::lock_recovered(groups_ref);
        groups.iter().map(|(k, g)| (k.clone(), g.clone())).collect()
    };

    for (key, group) in &group_snapshot {
        // Drain the idle queue for health checking.
        let workers_to_check: Vec<WorkerHandle> =
            { super::lock_recovered(&group.idle).drain(..).collect() };
        // We drained idle workers. Their permits are already consumed
        // (no one can acquire them). We'll re-add permits for healthy ones.

        let mut to_return = Vec::new();
        let mut restart_count = 0usize;
        let mut removed_count = 0usize;

        for mut worker in workers_to_check {
            // Check if process is alive
            if !worker.is_alive() {
                warn!(
                    target = %key.0.label(),
                    lang = %key.1,
                    engine_overrides = %key.2,
                    pid = %worker.pid(),
                    "Worker process died, scheduling restart"
                );
                removed_count += 1;
                restart_count += 1;
                // worker dropped here (SIGTERM+SIGKILL via WorkerHandle::Drop)
                continue;
            }

            // Health check via worker IPC
            match worker.health_check().await {
                Ok(_) => {
                    to_return.push(worker);
                }
                Err(e) => {
                    warn!(
                        target = %key.0.label(),
                        lang = %key.1,
                        engine_overrides = %key.2,
                        pid = %worker.pid(),
                        error = %e,
                        "Health check failed, scheduling restart"
                    );
                    removed_count += 1;
                    restart_count += 1;
                }
            }
        }

        // Return healthy workers
        {
            let returned = to_return.len();
            let mut idle = super::lock_recovered(&group.idle);
            for w in to_return {
                idle.push_back(w);
            }
            group.available.add_permits(returned);
        }

        group.record_worker_removed(removed_count);

        // Restart failed workers
        for _ in 0..restart_count {
            // Try to claim a global-cap permit for the restart. If
            // every permit has been grabbed by concurrent admissions
            // since the reaper refunded them, skip — a future
            // admission will spawn this worker on demand.
            let Some(restart_guard) =
                super::permit::SpawnPermitGuard::try_acquire_or_skip(&group.spawn_permits, || {
                    warn!(
                        target = %key.0.label(),
                        lang = %key.1,
                        engine_overrides = %key.2,
                        "Skipping reaper restart: global cap reached"
                    );
                })
            else {
                continue;
            };

            info!(
                target = %key.0.label(),
                lang = %key.1,
                engine_overrides = %key.2,
                "Restarting worker"
            );

            let _bootstrap_guard = group.bootstrap.lock().await;

            let config = WorkerConfig {
                python_path: pool_config.python_path.clone(),
                profile: key.0.profile_kind(),
                task: key.0.task(),
                lang: key.1.clone(),
                num_speakers: NumSpeakers(1),
                engine_overrides: key.2.clone(),
                test_echo: pool_config.test_echo,
                ready_timeout_s: pool_config.ready_timeout_s,
                verbose: pool_config.verbose,
                runtime: pool_config.runtime.clone(),
                audio_task_timeout_s: pool_config.audio_task_timeout_s,
                analysis_task_timeout_s: pool_config.analysis_task_timeout_s,
                test_delay_ms: pool_config.test_delay_ms,
            };

            match WorkerHandle::spawn(config).await {
                Ok(handle) => {
                    let pid = handle.pid();
                    group.total.fetch_add(1, Ordering::Relaxed);
                    super::lock_recovered(&group.idle).push_back(handle);
                    group.available.add_permits(1);
                    // Worker is officially counted; transfer permit
                    // lifetime to the worker's exit paths.
                    restart_guard.forget();
                    info!(
                        target = %key.0.label(),
                        lang = %key.1,
                        engine_overrides = %key.2,
                        pid = %pid,
                        "Worker restarted"
                    );
                }
                Err(e) => {
                    // restart_guard drops here, refunding the permit.
                    error!(
                        target = %key.0.label(),
                        lang = %key.1,
                        engine_overrides = %key.2,
                        error = %e,
                        "Failed to restart worker"
                    );
                }
            }
        }
    }

    // Clean up empty groups
    {
        let mut groups = super::lock_recovered(groups_ref);
        groups.retain(|_, g| g.total.load(Ordering::Relaxed) > 0);
    }
}

/// Memory-pressure-driven eviction pre-pass invoked by
/// [`run_health_check`]. Skips out cheaply when there's no pressure.
/// On pressure, samples idle workers' RSS via the same machinery
/// `rss_observer` uses for admission, picks victims via
/// [`idle_eviction::select_pressure_evictions`] (largest-RSS first),
/// and shuts them down with the same teardown sequence the
/// time-based eviction loop uses (`shutdown_in_place` →
/// `group.total.fetch_sub` → `SpawnPermitGuard::release_n`).
///
/// Workers that were idle at snapshot time but checked out before
/// the eviction commits are silently skipped — that's not a bug,
/// just a TOCTOU race where the worker is now busy serving a
/// request and the next eviction round will reconsider it if
/// pressure persists.
async fn pressure_evict_idle_workers_if_needed(groups_ref: &GroupsMap) {
    let available_mb = crate::worker::memory_guard::available_memory_mb();
    if available_mb >= idle_eviction::EVICTION_PRESSURE_THRESHOLD_MB {
        return;
    }
    let samples = idle_eviction::snapshot_idle_workers_with_rss(groups_ref);
    let to_evict = idle_eviction::select_pressure_evictions(
        samples,
        available_mb,
        idle_eviction::EVICTION_PRESSURE_THRESHOLD_MB,
    );
    if to_evict.is_empty() {
        return;
    }

    info!(
        available_mb,
        threshold_mb = idle_eviction::EVICTION_PRESSURE_THRESHOLD_MB,
        evict_count = to_evict.len(),
        "Memory pressure detected, evicting idle workers (largest-RSS first)"
    );

    for sample in to_evict {
        // Find and remove the matching worker from the group's idle
        // queue. Lock is held just long enough to remove the handle;
        // the async shutdown happens after `drop(idle)`.
        let group = &sample.group;
        let removed = {
            let mut idle = super::lock_recovered(&group.idle);
            let Some(pos) = idle.iter().position(|h| h.pid() == sample.pid) else {
                // Worker checked out since snapshot — leave it for
                // the next eviction round if pressure persists.
                continue;
            };
            // Consume the matching idle-permit. Failure means an
            // existing permit-vs-idle-handle invariant violation
            // upstream; surface it rather than silently masking.
            match group.available.try_acquire() {
                Ok(permit) => permit.forget(),
                Err(e) => warn!(
                    pid = %sample.pid,
                    error = %e,
                    "Idle queue had handle but no matching permit; eviction continuing without permit consumption"
                ),
            }
            let Some(handle) = idle.remove(pos) else {
                // VecDeque::remove(pos) only returns None if pos is
                // out of bounds, but we just got pos from
                // position(...) under the same lock. Treat as a
                // best-effort skip rather than a panic.
                continue;
            };
            handle
        };
        let mut worker = removed;
        let _ = worker.shutdown_in_place().await;
        group.record_worker_removed(1);
        info!(
            pid = %sample.pid,
            rss_mb = sample.rss_mb,
            target = %sample.key.0.label(),
            lang = %sample.key.1,
            "Evicted idle worker for memory pressure"
        );
    }
}

#[cfg(test)]
mod saturation_log_tests {
    use super::should_log_saturation;

    /// A 30-minute saturation window on ming captured 664,937 rejection
    /// events (BUG-028). With logarithmic gating, the same window emits
    /// at most one WARN per power of two — about 20 events, not 664k.
    /// This test pins the schedule.
    #[test]
    fn logs_at_one_two_four_and_powers_of_two() {
        for &n in &[1u64, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 1_048_576] {
            assert!(
                should_log_saturation(n),
                "should log at power-of-two rejection count {n}"
            );
        }
    }

    #[test]
    fn does_not_log_at_non_power_of_two_counts() {
        for &n in &[3u64, 5, 6, 7, 9, 15, 17, 100, 999, 664_937] {
            assert!(
                !should_log_saturation(n),
                "must not log at non-power-of-two rejection count {n}"
            );
        }
    }

    /// Across a high-volume saturation window, the logarithmic gate
    /// emits a logarithmic number of events. For any cap N in
    /// `[2^k, 2^(k+1))` the count is exactly `k + 1` (powers of two
    /// from 2^0 through 2^k).
    #[test]
    fn high_volume_window_collapses_to_log2_events() {
        let cap: u64 = 1_000_000;
        let emitted: u64 = (1..=cap).filter(|&n| should_log_saturation(n)).count() as u64;
        // 2^19 = 524,288 ≤ 1_000_000 < 2^20 = 1,048,576.
        assert_eq!(emitted, 20);
    }
}
