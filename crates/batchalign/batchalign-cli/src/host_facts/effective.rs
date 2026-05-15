//! `EffectiveConfig` — the result of merging operator overrides with
//! `recommend()`'s output, against the detected `HostFacts`.
//!
//! Every production code site that today reads from `ServerConfig`,
//! `PoolConfig`, or `WorkerRuntimeConfig` for one of the host-facts-
//! migrated knobs eventually moves to read from `EffectiveConfig`
//! (Phase C2 of the migration). This module owns the merge logic;
//! callers do not implement override-vs-recommendation resolution
//! themselves.
//!
//! Two types in this file:
//!
//! - **`ConfigOverrides`** — what the operator (or CLI) explicitly
//!   set. Each field is `Option<T>`; `Some(v)` means "use `v`",
//!   `None` means "fall through to the recommendation."
//!   Constructed independently of `ServerConfig`, so the merge logic
//!   does not depend on the legacy `0 = auto` sentinel idiom.
//!   Phase C2 adds `From<&ServerConfig>` impls knob-by-knob to
//!   bridge the legacy fields here.
//!
//! - **`EffectiveConfig`** — the resolved values. Constructed only
//!   via `EffectiveConfig::resolve(overrides, facts)`. Holds the
//!   detected `HostFacts` so per-command queries
//!   (`max_workers_per_job(command)`) can recompute the
//!   recommendation lazily without callers re-passing facts.
//!
//! See `talkbank/docs/investigations/2026-04-25-host-facts-architecture.md`
//! § Layer 3 for the merge-precedence rules and rationale.

use crate::api::{MemoryMb, ReleasedCommand};
use crate::config::ServerConfig;

use super::recommendations::{
    PerProfile, RecommendedKnobs, recommend, recommend_max_workers_per_job,
};
use super::{HostFacts, HostFactsSource, RealHostFactsSource};

/// Operator-supplied overrides, one per migrated knob. `None` means
/// "no override; use the recommendation." `Some(v)` means "use `v`."
///
/// A small dedicated type (rather than passing `&ServerConfig` directly
/// into `resolve`) keeps the merge logic independent of the current
/// `ServerConfig` shape. Phase C2's per-knob migrations populate this
/// struct via `From<&ServerConfig>` impls — the resolve function
/// itself does not change as those migrations land.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    /// Override for `gpu_thread_pool_size`. `None` recommends.
    pub gpu_thread_pool_size: Option<u32>,
    /// Override for `force_cpu`. `Some(true)` forces CPU even on
    /// CUDA-capable hosts (legitimate testing override); `Some(false)`
    /// requests GPU even on hosts where it's not functional (validator
    /// will warn). `None` recommends.
    pub force_cpu: Option<bool>,
    /// Override for `max_total_workers`. `None` recommends.
    pub max_total_workers: Option<u32>,
    /// Override for `max_concurrent_jobs`. `None` recommends.
    pub max_concurrent_jobs: Option<u32>,
    /// Override for the per-command `max_workers_per_job`. Today
    /// applied uniformly across commands (legacy `ServerConfig` has
    /// one knob, not per-command); a future refinement can split this
    /// per command without breaking the merge contract.
    pub max_workers_per_job: Option<u32>,
    /// Per-profile overrides for `max_workers_per_key`. Each profile
    /// can be overridden independently of the others.
    pub max_workers_per_key_by_profile: PerProfileOverrides,
    /// Override for `memory_gate_mb`. `None` recommends.
    pub memory_gate_mb: Option<MemoryMb>,
}

/// Lift a [`ServerConfig`] into a [`ConfigOverrides`].
///
/// `ServerConfig` is the on-disk YAML shape; `ConfigOverrides` is the
/// runtime input to [`EffectiveConfig::resolve`]. Each migrated knob
/// maps one-to-one; the single `max_workers_per_key` knob fans out
/// uniformly to all three per-profile slots (gpu/stanza/io) — matching
/// the legacy "one PoolConfig.max_workers_per_key applies to every
/// profile in the pool" semantics. Per-profile differentiation would
/// require a `ServerConfig` shape change.
///
/// Knobs that have not yet migrated to `Option<T>` (`force_cpu`)
/// stay `None` here; [`EffectiveConfig::resolve`] then derives them
/// from the host recommendation. This impl is the bridge that lets
/// every call site read from the resolved view without knowing
/// which knobs have crossed the migration boundary.
impl From<&ServerConfig> for ConfigOverrides {
    fn from(cfg: &ServerConfig) -> Self {
        Self {
            gpu_thread_pool_size: cfg.gpu_thread_pool_size,
            force_cpu: cfg.force_cpu,
            max_total_workers: cfg.max_total_workers,
            max_workers_per_job: cfg.max_workers_per_job,
            max_concurrent_jobs: cfg.max_concurrent_jobs,
            memory_gate_mb: cfg.memory_gate_mb,
            max_workers_per_key_by_profile: PerProfileOverrides {
                gpu: cfg.max_workers_per_key,
                stanza: cfg.max_workers_per_key,
                io: cfg.max_workers_per_key,
            },
        }
    }
}

/// Per-profile overrides for `max_workers_per_key`.
///
/// Mirrors the shape of `PerProfile<u32>` from `recommendations.rs`
/// but with `Option<u32>` per field — independent override per profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PerProfileOverrides {
    /// Override for the GPU profile worker count per key.
    pub gpu: Option<u32>,
    /// Override for the Stanza profile worker count per key.
    pub stanza: Option<u32>,
    /// Override for the IO profile worker count per key.
    pub io: Option<u32>,
}

/// Resolved per-host configuration values, post-merge.
///
/// Construction goes through [`EffectiveConfig::resolve`]. Holds the
/// `HostFacts` snapshot internally so the per-command
/// `max_workers_per_job` query can lazily compute the per-command
/// recommendation when no override is set — without forcing callers
/// to pass facts at every call site.
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    /// Resolved `gpu_thread_pool_size`.
    pub gpu_thread_pool_size: u32,
    /// Resolved `force_cpu`.
    pub force_cpu: bool,
    /// Resolved `max_total_workers`.
    pub max_total_workers: u32,
    /// Resolved `max_concurrent_jobs`.
    pub max_concurrent_jobs: u32,
    /// Resolved per-profile `max_workers_per_key`.
    pub max_workers_per_key_by_profile: PerProfile<u32>,
    /// Resolved `memory_gate_mb`.
    pub memory_gate_mb: MemoryMb,

    /// Operator override for `max_workers_per_job`, retained so
    /// `max_workers_per_job(command)` can apply it uniformly across
    /// commands. `None` means "fall through to the per-command
    /// recommendation computed from `host_facts`."
    max_workers_per_job_override: Option<u32>,

    /// Snapshot of the facts used to construct this config, retained
    /// for the per-command recommendation computation. Cloned once at
    /// resolve time; the struct is small.
    host_facts: HostFacts,
}

impl EffectiveConfig {
    /// Resolve effective values by merging operator overrides with
    /// the recommendation derived from `HostFacts`.
    ///
    /// Merge rule per knob: `override.unwrap_or(recommendation)`.
    /// Per-profile and per-command merges follow the same rule, applied
    /// per field / per command.
    ///
    /// Pure function: same `(overrides, facts)` always produces the
    /// same `EffectiveConfig`. No I/O, no time-dependence.
    pub fn resolve(overrides: &ConfigOverrides, facts: &HostFacts) -> Self {
        let r: RecommendedKnobs = recommend(facts);

        let max_workers_per_key_by_profile = PerProfile {
            gpu: overrides
                .max_workers_per_key_by_profile
                .gpu
                .unwrap_or(r.max_workers_per_key_by_profile.gpu),
            stanza: overrides
                .max_workers_per_key_by_profile
                .stanza
                .unwrap_or(r.max_workers_per_key_by_profile.stanza),
            io: overrides
                .max_workers_per_key_by_profile
                .io
                .unwrap_or(r.max_workers_per_key_by_profile.io),
        };

        Self {
            gpu_thread_pool_size: overrides
                .gpu_thread_pool_size
                .unwrap_or(r.gpu_thread_pool_size),
            force_cpu: overrides.force_cpu.unwrap_or(r.force_cpu),
            max_total_workers: overrides.max_total_workers.unwrap_or(r.max_total_workers),
            max_concurrent_jobs: overrides
                .max_concurrent_jobs
                .unwrap_or(r.max_concurrent_jobs),
            max_workers_per_key_by_profile,
            memory_gate_mb: overrides.memory_gate_mb.unwrap_or(MemoryMb(
                crate::worker::pool::memory_gate::MIN_FREE_MEMORY_MB,
            )),
            max_workers_per_job_override: overrides.max_workers_per_job,
            host_facts: facts.clone(),
        }
    }

    /// Convenience: detect live `HostFacts`, lift a `ServerConfig`
    /// into a `ConfigOverrides` via [`From<&ServerConfig>`], and
    /// resolve.
    ///
    /// This is the standard production entry point — the triple-step
    /// dance (detect → bridge → resolve) lives in one place so
    /// callers don't have to repeat it. The detection runs through
    /// [`RealHostFactsSource`] (millisecond-scale sysinfo poll) and
    /// is intended to be invoked at startup boundaries
    /// (`DispatchHostContext::from_store`, daemon serve, direct
    /// dispatch builders), not in per-job or per-request hot paths.
    /// Tests that need to exercise the same wiring against a
    /// synthesized fact set should call [`EffectiveConfig::resolve`]
    /// directly with a `MockHostFactsSource`-produced `HostFacts`.
    pub fn resolve_from_server_config(config: &ServerConfig) -> Self {
        let facts = RealHostFactsSource.detect();
        let overrides = ConfigOverrides::from(config);
        Self::resolve(&overrides, &facts)
    }

    /// Resolved `max_workers_per_job` for one command.
    ///
    /// When the operator set `max_workers_per_job` it wins for every
    /// command (matches the legacy single-knob `ServerConfig` shape).
    /// Otherwise the per-command recommendation is computed from the
    /// stored facts.
    pub fn max_workers_per_job(&self, command: &ReleasedCommand) -> u32 {
        self.max_workers_per_job_override
            .unwrap_or_else(|| recommend_max_workers_per_job(&self.host_facts, command))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{GpuPresence, test_helpers::apple_silicon_64gb};
    use super::*;

    fn cmd(name: &str) -> ReleasedCommand {
        ReleasedCommand::try_from(name).expect("test command literal must be valid")
    }

    // -------------------------------------------------------------------
    // Per-knob merge precedence: override Some → effective = override;
    // override None → effective = recommendation.
    // -------------------------------------------------------------------

    #[test]
    fn no_overrides_yields_recommendation() {
        let facts = apple_silicon_64gb();
        let effective = EffectiveConfig::resolve(&ConfigOverrides::default(), &facts);
        let r = recommend(&facts);
        assert_eq!(effective.gpu_thread_pool_size, r.gpu_thread_pool_size);
        assert_eq!(effective.force_cpu, r.force_cpu);
        assert_eq!(effective.max_total_workers, r.max_total_workers);
        assert_eq!(effective.max_concurrent_jobs, r.max_concurrent_jobs);
        assert_eq!(
            effective.max_workers_per_key_by_profile,
            r.max_workers_per_key_by_profile
        );
        assert_eq!(
            effective.memory_gate_mb,
            MemoryMb(crate::worker::pool::memory_gate::MIN_FREE_MEMORY_MB)
        );
    }

    #[test]
    fn gpu_thread_pool_size_override_wins() {
        let facts = apple_silicon_64gb();
        let overrides = ConfigOverrides {
            gpu_thread_pool_size: Some(7),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert_eq!(effective.gpu_thread_pool_size, 7);
    }

    /// Overriding `force_cpu` to `false` on a host whose recommendation
    /// is `true` survives the merge (the validator will warn). This
    /// test pins the merge contract; the warning lives in Phase D.
    #[test]
    fn force_cpu_override_to_false_survives_recommendation_true() {
        let facts = apple_silicon_64gb();
        assert!(recommend(&facts).force_cpu, "Apple Silicon recommends true");
        let overrides = ConfigOverrides {
            force_cpu: Some(false),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert!(!effective.force_cpu);
    }

    #[test]
    fn force_cpu_override_to_true_survives_recommendation_false() {
        let mut facts = apple_silicon_64gb();
        facts.gpu = GpuPresence::NvidiaCuda {
            device_count: 1,
            total_vram_mb: 24_000,
            driver_version: "555.42".into(),
        };
        assert!(!recommend(&facts).force_cpu, "CUDA recommends false");
        let overrides = ConfigOverrides {
            force_cpu: Some(true),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert!(effective.force_cpu);
    }

    #[test]
    fn max_total_workers_override_wins() {
        let facts = apple_silicon_64gb();
        let overrides = ConfigOverrides {
            max_total_workers: Some(2),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert_eq!(effective.max_total_workers, 2);
    }

    #[test]
    fn max_concurrent_jobs_override_wins() {
        let facts = apple_silicon_64gb();
        let overrides = ConfigOverrides {
            max_concurrent_jobs: Some(3),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert_eq!(effective.max_concurrent_jobs, 3);
    }

    #[test]
    fn memory_gate_mb_override_wins() {
        let facts = apple_silicon_64gb();
        let overrides = ConfigOverrides {
            memory_gate_mb: Some(MemoryMb(16_000)),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert_eq!(effective.memory_gate_mb, MemoryMb(16_000));
    }

    /// Per-profile override of `max_workers_per_key.gpu` does not
    /// affect the other profiles' recommendations.
    #[test]
    fn max_workers_per_key_per_profile_override_is_independent() {
        let facts = apple_silicon_64gb();
        let r = recommend(&facts).max_workers_per_key_by_profile;
        let overrides = ConfigOverrides {
            max_workers_per_key_by_profile: PerProfileOverrides {
                gpu: Some(2),
                stanza: None,
                io: None,
            },
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        assert_eq!(effective.max_workers_per_key_by_profile.gpu, 2);
        assert_eq!(effective.max_workers_per_key_by_profile.stanza, r.stanza);
        assert_eq!(effective.max_workers_per_key_by_profile.io, r.io);
    }

    // -------------------------------------------------------------------
    // Per-command max_workers_per_job: precomputed across
    // ReleasedCommand::ALL.
    // -------------------------------------------------------------------

    #[test]
    fn max_workers_per_job_no_override_matches_recommendation_per_command() {
        let facts = apple_silicon_64gb();
        let effective = EffectiveConfig::resolve(&ConfigOverrides::default(), &facts);
        for command in ReleasedCommand::ALL {
            assert_eq!(
                effective.max_workers_per_job(&command),
                recommend_max_workers_per_job(&facts, &command),
                "no-override mismatch for {command:?}"
            );
        }
    }

    /// One operator-set value applies uniformly to every command
    /// (matches the legacy single-knob `ServerConfig` shape).
    #[test]
    fn max_workers_per_job_override_applies_to_every_command() {
        let facts = apple_silicon_64gb();
        let overrides = ConfigOverrides {
            max_workers_per_job: Some(5),
            ..Default::default()
        };
        let effective = EffectiveConfig::resolve(&overrides, &facts);
        for command in ReleasedCommand::ALL {
            assert_eq!(
                effective.max_workers_per_job(&command),
                5,
                "override should apply to {command:?}"
            );
        }
    }

    /// Spot-check that lookup by `&ReleasedCommand` works against an
    /// arbitrary instance, not just iteration over `ALL`.
    #[test]
    fn max_workers_per_job_lookup_by_arbitrary_instance() {
        let facts = apple_silicon_64gb();
        let effective = EffectiveConfig::resolve(&ConfigOverrides::default(), &facts);
        let value = effective.max_workers_per_job(&cmd("transcribe"));
        assert!(value >= 1);
    }

    /// Pure-function contract: same inputs → same output.
    #[test]
    fn resolve_is_pure() {
        let facts = apple_silicon_64gb();
        let overrides = ConfigOverrides {
            gpu_thread_pool_size: Some(2),
            ..Default::default()
        };
        let a = EffectiveConfig::resolve(&overrides, &facts);
        let b = EffectiveConfig::resolve(&overrides, &facts);
        assert_eq!(a.gpu_thread_pool_size, b.gpu_thread_pool_size);
        assert_eq!(a.force_cpu, b.force_cpu);
        assert_eq!(a.max_total_workers, b.max_total_workers);
        assert_eq!(
            a.max_workers_per_job(&cmd("transcribe")),
            b.max_workers_per_job(&cmd("transcribe"))
        );
    }
}
