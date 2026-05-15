//! Memory tier classification and per-worker peak estimator.
//!
//! Per the Phase α + β specs, this module is the SOLE canonical source of:
//!   (a) per-tier per-profile worker startup envelopes (Principle 1)
//!   (b) per-command peak estimation (Principle 2;
//!       was `tier_aware_command_execution_budget_mb` in `batchalign::types::runtime`).
//!
//! Spec: docs/architecture/2026-05-10-tier-aware-memory-consolidation.md
//!       docs/architecture/2026-05-10-phase-beta-command-spec.md

use crate::api::MemoryMb;
use crate::worker_profile::WorkerProfile;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MemoryTierKind — RAM-tier enum
// ---------------------------------------------------------------------------

/// RAM-tier classification for adaptive memory budgets.
///
/// Detected once at server startup from total system RAM. All memory guard
/// parameters (startup reservations, host headroom, max workers) are derived
/// from the tier rather than from fixed constants. This allows batchalign3
/// to run on 16 GB laptops through 256 GB servers without manual tuning.
///
/// The Large and Fleet tiers reproduce the existing fixed constants from
/// `runtime_constants.toml` exactly, so fleet machines see zero behavior
/// change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTierKind {
    /// < 24 GB total RAM (laptops, CI runners)
    Small,
    /// 24–48 GB (workstations, Frodo)
    Medium,
    /// 48–128 GB (development servers)
    Large,
    /// > 128 GB (fleet servers like net)
    Fleet,
}

impl std::str::FromStr for MemoryTierKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "large" => Ok(Self::Large),
            "fleet" => Ok(Self::Fleet),
            _ => Err(format!(
                "unknown memory tier {s:?}; valid values: small, medium, large, fleet"
            )),
        }
    }
}

impl std::fmt::Display for MemoryTierKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Small => write!(f, "Small (<24 GB)"),
            Self::Medium => write!(f, "Medium (24-48 GB)"),
            Self::Large => write!(f, "Large (48-128 GB)"),
            Self::Fleet => write!(f, "Fleet (>128 GB)"),
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryTier — concrete per-tier parameters
// ---------------------------------------------------------------------------

/// Concrete memory budget parameters for a detected tier.
///
/// Constructed via [`MemoryTier::from_total_mb`] (pure, testable) or
/// [`MemoryTier::detect`] (reads system RAM via sysinfo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryTier {
    /// Which tier was selected.
    pub kind: MemoryTierKind,
    /// Total system RAM in MB (as detected).
    pub total_mb: u64,
    /// Host headroom reserve — the coordinator refuses reservations that
    /// would leave available RAM below this threshold.
    pub headroom_mb: MemoryMb,
    /// Startup reservation for a GPU worker (Whisper, Wave2Vec, speaker).
    pub gpu_startup_mb: MemoryMb,
    /// Startup reservation for a Stanza worker (morphosyntax, utseg, coref).
    pub stanza_startup_mb: MemoryMb,
    /// Startup reservation for an IO worker (translate, opensmile, avqi).
    pub io_startup_mb: MemoryMb,
    /// Suggested maximum concurrent workers across all profiles.
    pub max_suggested_workers: usize,
}

impl MemoryTier {
    /// Select a tier from total system RAM (in MB). Pure function — no
    /// sysinfo dependency, fully testable with arbitrary values.
    pub fn from_total_mb(total_mb: u64) -> Self {
        //                  (kind, headroom, gpu, stanza, io, max_workers)
        let (kind, headroom, gpu, stanza, io, max_workers) = if total_mb < 24_000 {
            (MemoryTierKind::Small, 2_000, 6_000, 3_000, 2_000, 1)
        } else if total_mb < 48_000 {
            // Medium: LazyProfile mode — GPU worker starts empty, models loaded
            // on demand. Startup reservation is just process overhead (3 GB),
            // not full model weight. Max 1 worker to prevent OOM on 32 GB.
            (MemoryTierKind::Medium, 4_000, 3_000, 6_000, 3_000, 1)
        } else if total_mb < 128_000 {
            // Large — matches existing TOML constants exactly
            (MemoryTierKind::Large, 8_000, 16_000, 12_000, 4_000, 4)
        } else {
            // Fleet — same budgets as Large, more workers
            (MemoryTierKind::Fleet, 8_000, 16_000, 12_000, 4_000, 8)
        };
        Self {
            kind,
            total_mb,
            headroom_mb: MemoryMb(headroom),
            gpu_startup_mb: MemoryMb(gpu),
            stanza_startup_mb: MemoryMb(stanza),
            io_startup_mb: MemoryMb(io),
            max_suggested_workers: max_workers,
        }
    }

    /// Detect the tier from actual system RAM via sysinfo.
    pub fn detect() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let total_mb = sys.total_memory() / (1024 * 1024);
        Self::from_total_mb(total_mb)
    }
}

// ---------------------------------------------------------------------------
// Per-worker peak estimator (Principle 2)
// ---------------------------------------------------------------------------

/// Profile-explicit per-worker peak estimator.
///
/// Phase α Layer-3 fix: clamp the fleet-conservative worst-case
/// (`worst_case`) to the tier's per-profile envelope. On Large/Fleet
/// the clamp doesn't bite (envelope == worst case); on Small/Medium
/// the clamp lets a memory-tight host actually run.
///
/// The "envelope" is the tier's per-profile startup reservation — see
/// [`WorkerProfile::startup_reservation_mb_for_tier`]. That method is
/// the canonical accessor; this estimator delegates to it.
pub fn estimate_per_worker_peak_mb_with_profile(
    worst_case: MemoryMb,
    profile: WorkerProfile,
    tier: &MemoryTier,
) -> MemoryMb {
    let envelope = profile.startup_reservation_mb_for_tier(tier);
    MemoryMb(worst_case.0.min(envelope.0))
}

/// Estimate one worker's peak RSS for a command, given the tier.
///
/// Phase α Layer-3 fix: clamp the fleet-conservative worst-case
/// (`worst_case`) to the tier's per-profile envelope. On Large/Fleet
/// the clamp doesn't bite (envelope == worst case); on Small/Medium
/// the clamp lets a memory-tight host actually run.
///
/// This is the canonical estimator per spec Principle 2. Admission
/// sites consult this; none recompute from primitives.
///
/// `is_gpu_heavy` is the GPU/Stanza classification — true iff the
/// command needs a GPU worker. Today the caller looks this up from
/// `runtime_constants.toml::gpu_heavy_commands` via the legacy adapter
/// `runtime::estimate_per_worker_peak_mb_legacy`. Phase β Task 4 will
/// replace that lookup with `command_spec_for(name).profile == Gpu`,
/// at which point this `bool` parameter retires.
pub fn estimate_per_worker_peak_mb(
    worst_case: MemoryMb,
    is_gpu_heavy: bool,
    tier: &MemoryTier,
) -> MemoryMb {
    let profile = if is_gpu_heavy {
        WorkerProfile::Gpu
    } else {
        WorkerProfile::Stanza
    };
    estimate_per_worker_peak_mb_with_profile(worst_case, profile, tier)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_profile::WorkerProfile;

    /// Phase α's Layer 3 fix: clamp the worst-case to the tier's
    /// per-profile envelope. On Large/Fleet the clamp doesn't bite;
    /// on Small the clamp is what lets a 16 GB host run.
    #[test]
    fn estimate_clamps_to_tier_envelope() {
        let small = MemoryTier::from_total_mb(16_000);
        let gpu_budget =
            estimate_per_worker_peak_mb_with_profile(MemoryMb(16_000), WorkerProfile::Gpu, &small);
        assert_eq!(gpu_budget.0, 6_000, "GPU clamps to Small.gpu_startup_mb");

        let stanza_budget = estimate_per_worker_peak_mb_with_profile(
            MemoryMb(12_000),
            WorkerProfile::Stanza,
            &small,
        );
        assert_eq!(stanza_budget.0, 3_000);

        let large = MemoryTier::from_total_mb(64_000);
        let gpu_budget_large =
            estimate_per_worker_peak_mb_with_profile(MemoryMb(16_000), WorkerProfile::Gpu, &large);
        assert_eq!(gpu_budget_large.0, 16_000, "Large does not clamp");
    }
}
