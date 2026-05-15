//! Live CPU-loadavg admission gate.
//!
//! # Why this exists
//!
//! For four months admission has been governed by static estimates —
//! tier-derived `max_concurrent_jobs`, recommender-output
//! `max_workers_per_key`, empirical caps on the recommender output —
//! and every estimate has turned out wrong on at least one
//! command/host combination. Today's measurement (2026-05-08) on ming
//! puts morphotag's contention knee at K=4 and align's at K=1: a
//! single static value cannot satisfy both.
//!
//! The 1-minute CPU load average answers the question the static
//! estimates were trying to approximate ("does this host have CPU
//! headroom for one more worker?") with a runtime measurement instead.
//! `getloadavg(3)` is exposed identically on Linux and macOS (verified
//! responsive on Apple Silicon, 2026-05-08); `sysinfo::System::load_average()`
//! wraps it. The gate refuses spawns when the 1-minute load is
//! already at or above the host's CPU count — the universal
//! "system is fully busy" boundary, no per-host tuning required.
//!
//! # Where this plugs in
//!
//! Called from
//! [`crate::worker::pool::WorkerPool::try_claim_spawn_slot`] before
//! the global-permit acquisition so a CPU-saturation refusal does
//! not consume (and immediately refund) a global-cap permit. This is
//! the first measurement-based admission predicate; future predicates
//! (per-job RSS observation, available-memory delta tracking) will
//! plug in alongside it.

/// Information about the load reading that triggered an admission
/// refusal. Carries both numbers so observability can report
/// "rejected at loadavg=18.4 against threshold=16" rather than just
/// a boolean. Not surfaced beyond the admission seam today; the
/// pool's `cpu_saturation_rejections_total` counter is the
/// metrics-layer surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CpuSaturated {
    /// 1-minute load average at the time of the check.
    pub(super) loadavg_1m: f64,
    /// Threshold the load was compared against.
    pub(super) threshold: f64,
}

/// Read the 1-minute load average via `sysinfo`.
///
/// `sysinfo::System::load_average()` is an associated function (no
/// instance state) that calls `getloadavg(3)` on Unix. The value is
/// the kernel's exponentially-weighted moving average of runnable
/// task count over the last minute. On Apple Silicon and Linux the
/// signal is faithful; on Windows `sysinfo` returns zero (the OS
/// has no equivalent), in which case the gate is effectively
/// disabled — acceptable because the Windows fleet doesn't run the
/// daemon today.
fn current_loadavg_1m() -> f64 {
    sysinfo::System::load_average().one
}

/// Loadavg threshold: the host's logical CPU count, full stop.
///
/// No env var, no config field, no operator override. The
/// rearch's premise is that resource policy must be mechanical
/// because there is no human reading or tuning configuration on
/// fleet hosts — "operator" is a fiction in this deployment.
/// The threshold is derived once from
/// `std::thread::available_parallelism()` at admission time; if
/// that query fails (vanishingly rare; would also break much of
/// the rest of the daemon), we fall back to the conservative
/// `1.0` so the gate remains active rather than silently
/// disabled.
pub(super) fn host_cpu_count_as_threshold() -> f64 {
    std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0)
}

/// Check whether the host is currently CPU-saturated against
/// `threshold`. Returns `Err(CpuSaturated)` when the 1-minute load
/// is at or above the threshold; otherwise `Ok(())`.
///
/// The threshold is passed in (rather than computed inside) so unit
/// tests can drive both branches deterministically without env-var
/// contention between parallel test cases.
pub(super) fn check_cpu_saturation(threshold: f64) -> Result<(), CpuSaturated> {
    let loadavg_1m = current_loadavg_1m();
    if loadavg_1m >= threshold {
        Err(CpuSaturated {
            loadavg_1m,
            threshold,
        })
    } else {
        Ok(())
    }
}

/// State-aware CPU admission check, mirroring
/// [`super::memory_gate::check_memory_saturation_with_state`].
///
/// `PoolGateState::ColdStart` admits unconditionally — the pool has
/// no workers to back-pressure against, so refusing the first spawn
/// just to relieve unrelated host load wedges the pool with no
/// recovery path. CPU pressure is back-pressure, not safety.
///
/// `PoolGateState::Warm` runs the loadavg check normally.
pub(super) fn check_cpu_saturation_with_state(
    state: super::memory_gate::PoolGateState,
    threshold: f64,
) -> Result<(), CpuSaturated> {
    if state == super::memory_gate::PoolGateState::ColdStart {
        return Ok(());
    }
    check_cpu_saturation(threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real loadavg is always non-negative; threshold of 0.0 makes
    /// the gate certain to refuse on any live machine. (On a freshly
    /// booted CI host loadavg can be exactly 0.0; the comparison is
    /// `>=` so the gate still rejects at the boundary, which is what
    /// we want — refuse to admit when the load equals the ceiling.)
    #[test]
    fn rejects_when_loadavg_meets_or_exceeds_threshold() {
        let result = check_cpu_saturation(0.0);
        let Err(saturated) = result else {
            panic!("expected CpuSaturated rejection at threshold=0.0");
        };
        assert!(
            saturated.loadavg_1m >= 0.0,
            "loadavg must be non-negative, got {}",
            saturated.loadavg_1m
        );
        assert_eq!(saturated.threshold, 0.0);
    }

    /// A threshold far above any realistic load means the gate must
    /// admit. The bound (1e9) is well past every credible
    /// kernel-reported load value, so this exercises the Ok branch
    /// without depending on host idleness.
    #[test]
    fn admits_when_threshold_far_above_load() {
        let result = check_cpu_saturation(1.0e9);
        assert!(
            result.is_ok(),
            "expected Ok at threshold=1e9, got {result:?}"
        );
    }

    /// The host-derived threshold must be a positive number on any
    /// machine the daemon runs on. The fallback (`1.0`) only fires
    /// if `available_parallelism()` returns Err, which would break
    /// most other things in the daemon long before this gate.
    #[test]
    fn host_threshold_is_positive_on_any_real_machine() {
        let t = host_cpu_count_as_threshold();
        assert!(t >= 1.0, "expected threshold >= 1.0, got {t}");
    }
}
