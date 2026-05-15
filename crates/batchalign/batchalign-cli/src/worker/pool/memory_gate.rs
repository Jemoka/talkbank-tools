//! Live available-memory admission gate.
//!
//! # The architectural principle
//!
//! Admission gates are **back-pressure, not safety**. Safety against
//! actual at-spawn OOM is one layer down: `memory_guard` (per-worker
//! RSS observation + kill) and the OS OOM killer. The admission gate
//! exists to slow down N+1 spawns when existing workers are
//! pressured — not to refuse the FIRST spawn on an empty pool, which
//! has no contention to back-pressure against.
//!
//! Earlier versions of this module conflated the two: a single rule
//! `available - reservation > floor` decided both cases, and on
//! memory-tight hosts (laptops, CI runners) it refused the cold-start
//! spawn, leaving the pool dead-on-arrival with no path to recover
//! (eviction has nothing to evict). That violated the rearch's stated
//! goal of "accurate, dynamic capability checking that doesn't block
//! weaker machines from doing work." See [`PoolGateState`] for the
//! cold-start vs warm distinction the gate now honors.
//!
//! # The two layers
//!
//! 1. **Cold-start bypass** — `PoolGateState::ColdStart` admits
//!    unconditionally. No back-pressure has anything to push
//!    against on an empty pool; the right place to surface
//!    host-too-small failures is `memory_guard` after spawn.
//! 2. **Warm-pool projection** — `PoolGateState::Warm` runs the
//!    `available - reservation > floor` check. Refusing here
//!    relieves real contention rather than wedging the pool.
//!
//! # The floor
//!
//! Tier-scaled, not fixed. See
//! [`host_min_free_mb_threshold_for_tier`]. The previous fixed
//! 2048 MB was right for Medium-tier workstations and wrong on every
//! other tier — too tight on laptops (50% of a 4 GB machine), too
//! loose on fleet (0.78% of a 256 GB machine).
//!
//! No env var, no `PoolConfig` field, no operator override. The
//! daemon is deployed by `pyinfra` from a Claude-generated
//! `server.yaml` that no human reads or edits.
//!
//! # Where this plugs in
//!
//! Called from
//! [`crate::worker::pool::WorkerPool::try_claim_spawn_slot`] as
//! Layer 0.5 — after the [`super::cpu_gate`] check, before the
//! global-permit acquisition. Memory and CPU rejections do not
//! consume permits; they refuse the spawn outright and let the
//! caller retry once the host frees up.

/// Minimum free memory in MB that must remain available for the OS
/// and non-batchalign processes on Medium-tier hosts (24-48 GB).
/// Used as the base for tier-scaled floors via
/// [`host_min_free_mb_threshold_for_tier`].
///
/// Visible at crate scope so the host-memory coordinator
/// (`crate::host_memory`) and the `ServerConfig` resolver
/// (`crate::types::config::resolve`) can reach the same constant
/// when their checks need a default headroom value. The
/// authoritative tier-aware floor goes through
/// [`host_min_free_mb_threshold_for_tier`].
pub(crate) const MIN_FREE_MEMORY_MB: u64 = 2048;

/// Whether the worker pool currently has any workers for the
/// (profile, lang, engine) class the admission check is for.
///
/// The principled architectural distinction underlying the runtime
/// admission gate's purpose: gates implement *back-pressure*, not
/// *safety*. Back-pressure has nothing to push against on an empty
/// pool, so a `ColdStart` admission must always be allowed —
/// refusing it leaves the pool dead-on-arrival on memory-tight
/// hosts (laptops, CI runners) with no path to ever spawn a
/// worker. Safety against actual at-spawn OOM lives one layer
/// down in `memory_guard` (per-worker RSS observation + kill) and
/// the OS OOM killer.
///
/// `Warm` admissions are the back-pressure case: at least one
/// worker exists for this class, so refusing the next spawn under
/// memory pressure relieves contention rather than wedging the
/// pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PoolGateState {
    /// The pool has no workers (and no concurrent spawn attempts)
    /// for the requested (profile, lang, engine) class. The
    /// memory gate must admit — there's nothing to back-pressure
    /// against, and refusing here means the pool never starts.
    ColdStart,
    /// At least one worker exists or is being spawned for the
    /// class. The memory gate applies normally; refuse N+1 under
    /// pressure.
    Warm,
}

/// Information about the memory reading that triggered an admission
/// refusal. Carries the live reading, the reservation the projection
/// was made against, and the floor that was breached, so
/// observability can report
/// "available=10240 MB minus new-worker reservation=12000 MB would
/// leave -1760 MB; floor=2048 MB" rather than just a boolean. Not
/// surfaced beyond the admission seam today; the pool's
/// `memory_constrained_rejections_total` counter is the metrics-layer
/// surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MemoryConstrained {
    /// Available memory in MB at the time of the check.
    pub(super) available_mb: u64,
    /// The new worker's projected reservation in MB. Subtracted
    /// from `available_mb` to compute the post-spawn projection.
    /// `0` means "no reservation projection" (i.e., the gate is
    /// only enforcing the floor, equivalent to the original
    /// pre-Mode-A behavior).
    pub(super) reservation_mb: u64,
    /// Floor the projected post-spawn memory was compared against.
    pub(super) threshold_mb: u64,
}

/// Read the host's currently-available memory in MB.
///
/// Delegates to [`crate::worker::memory_guard::available_memory_mb`]
/// so the admission gate and the in-spawn `memory_guard` agree on
/// what "available" means — important on macOS, where naive
/// `sysinfo::available_memory()` undercounts by excluding inactive
/// pages that the kernel can reclaim on demand.
fn current_available_memory_mb() -> u64 {
    crate::worker::memory_guard::available_memory_mb()
}

/// Tier-scaled minimum-free floor in MB. The 2048-MB hardcode the
/// rearch shipped was wrong on every host: too tight on laptops
/// (50% of a 4 GB machine), too loose on fleet (0.78% of a 256 GB
/// machine). The principled shape: floor reflects the host's safe
/// headroom budget, just like the per-profile reservation already
/// does via [`MemoryTier::stanza_startup_mb`] and friends.
///
/// Numbers chosen so a Small laptop (16 GB) sees ~6% headroom and
/// a Fleet (256 GB) sees ~1.5%:
///
/// | Tier   | Range      | Floor   |
/// |--------|------------|---------|
/// | Small  | < 24 GB    | 1024 MB |
/// | Medium | 24–48 GB   | 2048 MB |
/// | Large  | 48–128 GB  | 4096 MB |
/// | Fleet  | > 128 GB   | 4096 MB |
///
/// Returned by a function (not inlined as a constant) so unit
/// tests can audit "what number is the production gate using"
/// without duplicating tier-arithmetic at every call site.
pub(super) fn host_min_free_mb_threshold_for_tier(tier: &crate::types::runtime::MemoryTier) -> u64 {
    use crate::types::runtime::MemoryTierKind;
    match tier.kind {
        MemoryTierKind::Small => 1024,
        MemoryTierKind::Medium => 2048,
        MemoryTierKind::Large | MemoryTierKind::Fleet => 4096,
    }
}

/// State-aware admission check.
///
/// Implements the principled rearch follow-up: the memory gate is
/// back-pressure, not safety. On `PoolGateState::ColdStart` (no
/// existing worker for the class), always admit — the pool has
/// nothing to push back against, and refusing here leaves the
/// pool dead-on-arrival on memory-tight hosts. Safety against
/// actual at-spawn OOM is `memory_guard`'s job (per-worker RSS
/// observation + kill) and the OS OOM killer's; this function
/// must not double up.
///
/// On `PoolGateState::Warm`, applies the original projection:
/// `Err(MemoryConstrained)` when `available_mb < reservation_mb +
/// threshold_mb`; otherwise `Ok(())`. Production callers pass the
/// new worker's `startup_reservation_mb_for_tier` so the
/// projection accounts for the worker's anticipated load.
/// Reservation is a static per-profile estimate — a known
/// imperfection that the rearch's Mode B (RSS observation of
/// running same-profile peers) refines once at least one worker
/// exists. Cold-start by definition has no peers to observe.
pub(super) fn check_memory_saturation_with_state(
    state: PoolGateState,
    threshold_mb: u64,
    reservation_mb: u64,
) -> Result<(), MemoryConstrained> {
    if state == PoolGateState::ColdStart {
        return Ok(());
    }
    let available_mb = current_available_memory_mb();
    let projected_after_spawn_mb = available_mb.saturating_sub(reservation_mb);
    if projected_after_spawn_mb <= threshold_mb {
        Err(MemoryConstrained {
            available_mb,
            reservation_mb,
            threshold_mb,
        })
    } else {
        Ok(())
    }
}

/// Legacy entry point for tests that pre-date the state-aware
/// variant. Behaves as `check_memory_saturation_with_state(Warm,
/// ...)` so the existing rejection-branch tests continue to fire
/// without modification. Production callers must use the
/// state-aware variant.
#[cfg(test)]
fn check_memory_saturation(
    threshold_mb: u64,
    reservation_mb: u64,
) -> Result<(), MemoryConstrained> {
    check_memory_saturation_with_state(PoolGateState::Warm, threshold_mb, reservation_mb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Medium-tier floor must equal exactly 2048 MB — the
    /// rearch's original number, preserved for the workstation case
    /// (Frodo / dev hosts). If anyone ever changes the Medium tier's
    /// floor, the change should be deliberate enough to update this
    /// test alongside it.
    #[test]
    fn medium_tier_floor_is_2048_mb() {
        let medium = crate::types::runtime::MemoryTier::from_total_mb(32_000);
        assert_eq!(MIN_FREE_MEMORY_MB, 2048);
        assert_eq!(host_min_free_mb_threshold_for_tier(&medium), 2048);
    }

    /// Threshold of `u64::MAX` makes the gate certain to refuse with
    /// any reservation: any real machine has finite memory and the
    /// post-spawn projection trivially falls at or below the floor.
    /// Exercises the rejection branch deterministically.
    #[test]
    fn rejects_when_floor_alone_exceeds_available() {
        let result = check_memory_saturation(u64::MAX, 0);
        let Err(constrained) = result else {
            panic!("expected MemoryConstrained rejection at threshold=u64::MAX");
        };
        assert_eq!(constrained.threshold_mb, u64::MAX);
        assert_eq!(constrained.reservation_mb, 0);
        assert!(constrained.available_mb <= u64::MAX);
    }

    /// Reservation of `u64::MAX` with floor=0 also forces refusal:
    /// the projected post-spawn memory saturates at 0, which is at
    /// or below the floor. Pins the projection arithmetic.
    #[test]
    fn rejects_when_reservation_alone_exceeds_available() {
        let result = check_memory_saturation(0, u64::MAX);
        let Err(constrained) = result else {
            panic!("expected rejection when reservation > available");
        };
        assert_eq!(constrained.reservation_mb, u64::MAX);
        assert_eq!(constrained.threshold_mb, 0);
    }

    /// Floor=0 and reservation=0 means the predicate reduces to
    /// "is available > 0?" — won't fire on a running machine.
    /// Exercises the admit branch without depending on host
    /// idleness.
    #[test]
    fn admits_when_floor_and_reservation_are_zero() {
        let result = check_memory_saturation(0, 0);
        assert!(
            result.is_ok(),
            "expected Ok at floor=0,reservation=0 on a running host, got {result:?}"
        );
    }

    /// Confirms the projection arithmetic: with reservation 1 MB and
    /// floor 0, the gate must admit (any real host has more than
    /// 1 MB free).
    #[test]
    fn admits_when_projected_post_spawn_well_above_floor() {
        let result = check_memory_saturation(0, 1);
        assert!(
            result.is_ok(),
            "expected Ok at floor=0,reservation=1 on a running host, got {result:?}"
        );
    }

    // ---------------------------------------------------------------
    // Layer 2 (cold-start bootstrap rule + tier-scaled floor).
    //
    // The rearch's stated goal is "accurate, dynamic capability
    // checking that doesn't block weaker machines from doing work."
    // The runtime admission gate landed in the rearch refused the
    // FIRST worker on memory-constrained hosts because the
    // empty-pool case went through the same projection as the
    // populated-pool case. That left the pool dead-on-arrival on
    // hosts where `available - reservation < floor`, with no path to
    // ever spawn a worker (eviction has nothing to evict).
    //
    // The principled architecture: admission gates are
    // back-pressure, not safety. Safety lives in memory_guard
    // (per-worker RSS observation + kill) and the OS OOM killer.
    // The admission gate's job is to slow down N+1 spawns when
    // existing workers are pressured — not to refuse the FIRST
    // spawn on an empty pool, which has no contention to
    // back-pressure against.
    //
    // Plus the floor: 2048 MB is wrong on both extremes. On a 4 GB
    // host it's 50% of total RAM (silly). On a 256 GB fleet host
    // it's 0.78% (silly). The floor needs to scale with tier just
    // like the per-profile reservation does.
    // ---------------------------------------------------------------

    use crate::types::runtime::{MemoryTier, MemoryTierKind};

    /// Cold-start: the pool has no workers (and no concurrent spawn
    /// attempts) for this class. Even with a u64::MAX threshold that
    /// would otherwise force refusal, the admission gate must allow
    /// the spawn — back-pressure semantics don't apply when there's
    /// nothing to push back against.
    #[test]
    fn cold_start_pool_admits_first_worker_under_memory_pressure() {
        let result = check_memory_saturation_with_state(PoolGateState::ColdStart, u64::MAX, 0);
        assert!(
            result.is_ok(),
            "cold-start admission must bypass the floor: a pool with no \
             workers cannot be 'saturated' and refusing the first spawn \
             leaves the pool dead-on-arrival. got {result:?}"
        );
    }

    /// Cold-start with a non-trivial reservation also admits. Even
    /// when the static reservation projection is hostile, the cold
    /// path bypasses — memory_guard handles actual at-spawn OOM.
    #[test]
    fn cold_start_pool_admits_under_reservation_pressure_too() {
        let result = check_memory_saturation_with_state(PoolGateState::ColdStart, 0, u64::MAX);
        assert!(
            result.is_ok(),
            "cold-start must admit regardless of reservation projection. \
             got {result:?}"
        );
    }

    /// Warm pool: at least one worker exists for this class. The
    /// admission gate applies normally — refuse the N+1 spawn under
    /// memory pressure. Preserves the rearch's runtime-admission
    /// behavior for the legitimate saturation case.
    #[test]
    fn warm_pool_still_refuses_under_memory_pressure() {
        let result = check_memory_saturation_with_state(PoolGateState::Warm, u64::MAX, 0);
        assert!(
            result.is_err(),
            "warm-pool admission must still refuse N+1 under memory \
             pressure — back-pressure is the gate's whole job once the \
             pool has workers to push back against. got {result:?}"
        );
    }

    /// Warm pool with reservation pressure: same back-pressure
    /// semantics. Preserves existing rejection behavior for callers
    /// that pass a real reservation.
    #[test]
    fn warm_pool_refuses_when_reservation_alone_exceeds_available() {
        let result = check_memory_saturation_with_state(PoolGateState::Warm, 0, u64::MAX);
        assert!(
            result.is_err(),
            "warm-pool admission must refuse when reservation exceeds \
             available. got {result:?}"
        );
    }

    /// Warm pool with no pressure admits — the gate isn't there to
    /// refuse work that fits, only to hold back work that doesn't.
    #[test]
    fn warm_pool_admits_when_floor_and_reservation_are_zero() {
        let result = check_memory_saturation_with_state(PoolGateState::Warm, 0, 0);
        assert!(
            result.is_ok(),
            "warm-pool admission with no pressure must admit. got {result:?}"
        );
    }

    /// Floor scales with memory tier. The 2048-MB hardcode was
    /// wrong on every host: too tight on laptops (50% of a 4 GB
    /// machine), too loose on fleet (0.78% of a 256 GB machine).
    /// The principled shape: tier-aware floors that reflect the
    /// host's safe-headroom budget. Numbers chosen so a Small
    /// laptop (16 GB) sees ~6% headroom and a Fleet (256 GB) sees
    /// ~1.5%.
    #[test]
    fn host_min_free_mb_threshold_scales_with_tier() {
        let small = MemoryTier::from_total_mb(16_000);
        let medium = MemoryTier::from_total_mb(32_000);
        let large = MemoryTier::from_total_mb(64_000);
        let fleet = MemoryTier::from_total_mb(256_000);

        assert_eq!(small.kind, MemoryTierKind::Small);
        assert_eq!(medium.kind, MemoryTierKind::Medium);
        assert_eq!(large.kind, MemoryTierKind::Large);
        assert_eq!(fleet.kind, MemoryTierKind::Fleet);

        assert_eq!(host_min_free_mb_threshold_for_tier(&small), 1024);
        assert_eq!(host_min_free_mb_threshold_for_tier(&medium), 2048);
        assert_eq!(host_min_free_mb_threshold_for_tier(&large), 4096);
        assert_eq!(host_min_free_mb_threshold_for_tier(&fleet), 4096);
    }
}
