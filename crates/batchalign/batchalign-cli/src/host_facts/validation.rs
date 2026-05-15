//! Config-vs-host-facts validation.
//!
//! At server startup, after `EffectiveConfig::resolve_from_server_config`
//! has merged operator overrides with the host-facts recommendation,
//! [`validate`] inspects the *original* operator overrides against the
//! detected facts and reports two kinds of finding:
//!
//! - [`ConfigWarning`] — the override is suboptimal but the server can
//!   still run. Surfaced as `tracing::warn!` lines at startup so the
//!   operator notices, but never blocks. Conservative-vs-recommendation
//!   mismatches (operator over-conservative — fewer workers than the
//!   host could support) are intentionally **silent**: the operator
//!   knows their host better than `recommend()` does, and silence is
//!   the right ergonomics for "intentionally cautious."
//! - [`ConfigError`] — the override would deterministically crash or
//!   produce wrong output (e.g., per-job worker × concurrent jobs ×
//!   peak RAM exceeds physical RAM). Reserved for cases where running
//!   would clearly fail; the server refuses to start with a message
//!   that includes the recommendation.
//!
//! See `talkbank/docs/investigations/2026-04-25-host-facts-architecture.md`
//! § Layer 4 for the validate-warns-mostly rationale (decision Q2).
//!
//! ## Why not use `EffectiveConfig` directly
//!
//! Validation needs to know which knobs the operator *explicitly set*
//! versus which fell through to the recommendation, because the
//! "operator contradicts facts" warnings only fire on explicit
//! overrides. `EffectiveConfig` has already merged the two, so the
//! distinction is gone there. `validate` reads from `ServerConfig`
//! (where `Some(_)` means explicit, `None` means deferred to
//! recommendation) plus `HostFacts` directly.

use std::fmt;

use super::recommendations::{
    recommend_force_cpu, recommend_max_concurrent_jobs, recommend_max_total_workers,
    worst_case_per_job_peak_ram_mb,
};
use super::{GpuPresence, HostFacts};
use crate::config::ServerConfig;

/// One non-fatal finding from [`validate`].
///
/// Each variant carries the configured value, the relevant detected
/// fact, and the recommended alternative — the variant alone is enough
/// to render a self-explaining operator message via the [`Display`]
/// impl. New variants must follow the same shape so tracing
/// integration stays uniform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    /// Operator set `gpu_thread_pool_size` greater than 1 on a host
    /// whose GPU is not functional for batchalign (Apple Silicon MPS,
    /// no GPU at all, CUDA host with `device_count == 0`). Above-1 has
    /// no benefit on CPU-only execution and adds in-process contention
    /// without any parallelism gain.
    GpuThreadPoolSizeAboveOneOnCpu {
        /// The operator's configured value.
        configured: u32,
        /// The recommended value for this host (always 1 when the GPU
        /// is non-functional, so the message is self-explaining).
        recommended: u32,
    },
    /// Operator set `max_concurrent_jobs` higher than the host-facts
    /// recommendation derived from `ram_total_mb` and CPU
    /// availability. The configured value will admit more parallel
    /// jobs than the host can comfortably support, increasing the
    /// risk of memory-pressure stalls and worker OOMs.
    ///
    /// Conservative-vs-recommendation (configured `<=` recommended)
    /// is intentionally silent; only over-eager configurations
    /// warn.
    MaxConcurrentJobsAboveRamBudget {
        /// The operator's configured value.
        configured: u32,
        /// The recommended value for this host (derived from
        /// `recommend_max_concurrent_jobs`).
        recommended: u32,
    },
    /// Operator set `max_total_workers` higher than the host-facts
    /// recommendation derived from `ram_total_mb`. The configured
    /// ceiling permits more concurrent worker processes than physical
    /// RAM can support, increasing the risk of OOM under sustained
    /// load. Conservative-vs-recommendation is silent.
    MaxTotalWorkersAboveRamBudget {
        /// The operator's configured value.
        configured: u32,
        /// The recommended value for this host (derived from
        /// `recommend_max_total_workers`).
        recommended: u32,
    },
    /// Operator set `force_cpu = false` on a host whose GPU is not
    /// functional for batchalign. The configured value asserts
    /// "use the GPU" but the GPU pipeline cannot proceed — the
    /// worker will still fall back to CPU at runtime, but the
    /// asserted intent is wrong. Common cause: an old `server.yaml`
    /// from a CUDA host copied to an Apple Silicon host.
    ForceCpuFalseOnNonFunctionalGpu {
        /// The detected GPU presence (so the operator sees what
        /// "non-functional" means in their case). Carried as the
        /// typed enum rather than a Debug-formatted string so JSON
        /// consumers see structured data and the Display impl can
        /// format on demand.
        gpu: GpuPresence,
    },
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GpuThreadPoolSizeAboveOneOnCpu {
                configured,
                recommended,
            } => write!(
                f,
                "gpu_thread_pool_size={configured} on a host with no functional GPU; \
                 recommended value is {recommended}. The configured threads will \
                 contend for one CPU-bound model process without parallelism gain. \
                 Set `gpu_thread_pool_size: {recommended}` in server.yaml or omit \
                 the field to use the host-aware recommendation.",
            ),
            Self::MaxConcurrentJobsAboveRamBudget {
                configured,
                recommended,
            } => write!(
                f,
                "max_concurrent_jobs={configured} exceeds the host-facts \
                 recommendation of {recommended}. The configured value admits \
                 more parallel jobs than this host can comfortably support; \
                 expect memory-pressure stalls and worker OOMs under load. \
                 Set `max_concurrent_jobs: {recommended}` in server.yaml or \
                 omit the field to use the host-aware recommendation.",
            ),
            Self::MaxTotalWorkersAboveRamBudget {
                configured,
                recommended,
            } => write!(
                f,
                "max_total_workers={configured} exceeds the host-facts \
                 recommendation of {recommended} (derived from ram_total_mb / \
                 6 GB per worker, clamped to [2, 32]). The configured ceiling \
                 permits more concurrent worker processes than physical RAM \
                 can support; expect OOMs under sustained load. Set \
                 `max_total_workers: {recommended}` in server.yaml or omit \
                 the field to use the host-aware recommendation.",
            ),
            Self::ForceCpuFalseOnNonFunctionalGpu { gpu } => write!(
                f,
                "force_cpu=false is set, but this host's GPU is not \
                 functional for batchalign ({gpu:?}). The asserted \
                 intent is wrong — the worker will fall back to CPU at \
                 runtime regardless. Remove `force_cpu` from server.yaml \
                 (the host-facts recommendation will set it correctly) or \
                 set `force_cpu: true` to make the intent explicit.",
            ),
        }
    }
}

/// A fatal finding from [`validate`]. Server refuses to start.
///
/// Errors are reserved for cases where the configuration would
/// deterministically crash or produce wrong output. The plan calls
/// for further variants (e.g., per-command peak-RAM checks) as the
/// host-facts model gains richer fact shapes; today's single variant
/// catches the most common deploy-time mistake — over-eager
/// `max_concurrent_jobs` on a low-RAM host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Operator's `max_concurrent_jobs` value, when multiplied by
    /// the worst-case per-job peak RAM (the heaviest worker
    /// profile, today GPU at 16 GB), exceeds the host's physical
    /// RAM. There is no scheduling outcome that fits — every
    /// jobset that uses the heaviest profile would OOM. The
    /// server refuses to start; the operator must drop
    /// `max_concurrent_jobs` (or omit it to use the recommendation,
    /// which is by construction safe) before the daemon can boot.
    MaxConcurrentJobsWouldDeterministicallyOom {
        /// The operator's configured value.
        configured: u32,
        /// Detected total RAM in MB.
        ram_total_mb: u64,
        /// Worst-case per-job peak RAM (in MB) used in the check.
        per_job_peak_mb: u64,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxConcurrentJobsWouldDeterministicallyOom {
                configured,
                ram_total_mb,
                per_job_peak_mb,
            } => write!(
                f,
                "max_concurrent_jobs={configured} would deterministically \
                 OOM this host: worst-case per-job peak RAM is \
                 {per_job_peak_mb} MB and {configured} × {per_job_peak_mb} \
                 = {} MB exceeds the detected ram_total_mb of \
                 {ram_total_mb}. Drop `max_concurrent_jobs` from \
                 server.yaml (the host-facts recommendation is by \
                 construction safe) or set a value such that \
                 max_concurrent_jobs × {per_job_peak_mb} <= {ram_total_mb}. \
                 To run a single job locally without the daemon's memory \
                 gate at all, invoke `batchalign3 <command> --sequential` \
                 — `--sequential` runs the command in-process with no \
                 daemon, no concurrent-job admission, and no memory gate.",
                u64::from(*configured) * per_job_peak_mb,
            ),
        }
    }
}

/// Result bundle from one [`validate`] call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigValidation {
    /// Non-fatal findings; surface but do not block startup.
    pub warnings: Vec<ConfigWarning>,
    /// Fatal findings; refuse startup. Empty in the common case.
    pub errors: Vec<ConfigError>,
}

impl ConfigValidation {
    /// Whether validation found at least one fatal finding.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Inspect operator overrides in `cfg` against detected `facts` and
/// report any contradictions.
///
/// The function is pure: same `(cfg, facts)` always produces the same
/// `ConfigValidation`. No I/O, no time-dependence, no logging. Callers
/// are responsible for converting findings to `tracing` lines or
/// startup errors.
pub fn validate(cfg: &ServerConfig, facts: &HostFacts) -> ConfigValidation {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    if let Some(configured) = cfg.gpu_thread_pool_size
        && configured > 1
        && !facts.gpu.is_functional_for_batchalign()
    {
        warnings.push(ConfigWarning::GpuThreadPoolSizeAboveOneOnCpu {
            configured,
            recommended: 1,
        });
    }

    if let Some(configured) = cfg.max_concurrent_jobs {
        let recommended = recommend_max_concurrent_jobs(facts);
        if configured > recommended {
            warnings.push(ConfigWarning::MaxConcurrentJobsAboveRamBudget {
                configured,
                recommended,
            });
        }
        // Worst-case per-job × N > ram is the deterministic-OOM
        // case: even if every job uses the heaviest worker profile,
        // physical RAM cannot accommodate. This is an error
        // (refuses startup) rather than a warning because the
        // operator has no scheduling option that fits — the
        // configured cap is incorrect by construction.
        let tier = crate::types::runtime::MemoryTier::from_total_mb(facts.ram_total_mb);
        let per_job_peak_mb = worst_case_per_job_peak_ram_mb(&tier);
        if u64::from(configured) * per_job_peak_mb > facts.ram_total_mb {
            errors.push(ConfigError::MaxConcurrentJobsWouldDeterministicallyOom {
                configured,
                ram_total_mb: facts.ram_total_mb,
                per_job_peak_mb,
            });
        }
    }

    if let Some(configured) = cfg.max_total_workers {
        let recommended = recommend_max_total_workers(facts);
        if configured > recommended {
            warnings.push(ConfigWarning::MaxTotalWorkersAboveRamBudget {
                configured,
                recommended,
            });
        }
    }

    // `force_cpu = Some(false)` on a host whose recommendation is
    // `true` (non-functional GPU) is the contradiction to flag.
    // `Some(true)` on a CUDA host is the symmetric case but it's
    // operationally fine — the worker just doesn't use the GPU,
    // which is a legitimate testing affordance — so it's silent.
    // `None` defers to the recommendation by construction; never
    // contradicts.
    if cfg.force_cpu == Some(false) && recommend_force_cpu(facts) {
        warnings.push(ConfigWarning::ForceCpuFalseOnNonFunctionalGpu {
            gpu: facts.gpu.clone(),
        });
    }

    ConfigValidation { warnings, errors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_facts::test_helpers::{apple_silicon_64gb, linux_cuda_24gb};

    /// RED -> GREEN: `gpu_thread_pool_size: Some(4)` on Apple Silicon
    /// (non-functional GPU) must produce one
    /// `GpuThreadPoolSizeAboveOneOnCpu` warning carrying the
    /// configured and recommended values.
    #[test]
    fn warns_when_gpu_thread_pool_size_above_one_on_apple_silicon() {
        let cfg = ServerConfig {
            gpu_thread_pool_size: Some(4),
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert_eq!(
            result.warnings,
            vec![ConfigWarning::GpuThreadPoolSizeAboveOneOnCpu {
                configured: 4,
                recommended: 1,
            }],
        );
        assert!(result.errors.is_empty());
    }

    /// `gpu_thread_pool_size: Some(1)` on Apple Silicon: the operator
    /// matches the recommendation; no warning fires.
    #[test]
    fn silent_when_gpu_thread_pool_size_matches_recommendation_on_cpu() {
        let cfg = ServerConfig {
            gpu_thread_pool_size: Some(1),
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// `gpu_thread_pool_size: Some(4)` on a CUDA host (functional
    /// GPU): recommended value is also 4, so no contradiction fires.
    #[test]
    fn silent_when_gpu_thread_pool_size_above_one_on_cuda() {
        let cfg = ServerConfig {
            gpu_thread_pool_size: Some(4),
            ..Default::default()
        };
        let result = validate(&cfg, &linux_cuda_24gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// `gpu_thread_pool_size: None` (default — no operator override)
    /// never fires the warning, even on a CPU-only host. The whole
    /// point of the warning is to flag *operator overrides* that
    /// contradict facts; the recommendation is by construction
    /// consistent with the facts.
    #[test]
    fn silent_when_gpu_thread_pool_size_is_none_on_cpu() {
        let cfg = ServerConfig {
            gpu_thread_pool_size: None,
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    // -----------------------------------------------------------------
    // MaxConcurrentJobsAboveRamBudget
    // -----------------------------------------------------------------

    /// Apple Silicon at 64 GB RAM has a recommendation around 4
    /// concurrent jobs (clamp(ram_total / 6GB, 2, 32) = 10, then
    /// further clamped by `auto_max_concurrent_jobs` formula). An
    /// operator setting `max_concurrent_jobs = 99` is well above any
    /// reasonable budget; the warning must fire.
    #[test]
    fn warns_when_max_concurrent_jobs_exceeds_recommendation() {
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(99),
            ..Default::default()
        };
        let facts = apple_silicon_64gb();
        let recommended = super::recommend_max_concurrent_jobs(&facts);
        let result = validate(&cfg, &facts);
        assert_eq!(
            result.warnings,
            vec![ConfigWarning::MaxConcurrentJobsAboveRamBudget {
                configured: 99,
                recommended,
            }],
        );
    }

    /// Operator at exactly the recommendation: silent (boundary
    /// condition; `>` not `>=`).
    #[test]
    fn silent_when_max_concurrent_jobs_equals_recommendation() {
        let facts = apple_silicon_64gb();
        let recommended = super::recommend_max_concurrent_jobs(&facts);
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(recommended),
            ..Default::default()
        };
        let result = validate(&cfg, &facts);
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// Conservative-vs-recommendation is intentionally silent — the
    /// operator knows their host better than `recommend()` does, and
    /// silence is the right ergonomics for "intentionally cautious."
    #[test]
    fn silent_when_max_concurrent_jobs_below_recommendation() {
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(1),
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// `None` (no operator override) never fires; the recommendation
    /// is by construction consistent with the facts.
    #[test]
    fn silent_when_max_concurrent_jobs_is_none() {
        let cfg = ServerConfig {
            max_concurrent_jobs: None,
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn max_concurrent_jobs_warning_display_includes_both_values() {
        let warning = ConfigWarning::MaxConcurrentJobsAboveRamBudget {
            configured: 99,
            recommended: 4,
        };
        let rendered = format!("{warning}");
        assert!(
            rendered.contains("max_concurrent_jobs=99"),
            "rendered: {rendered}"
        );
        assert!(
            rendered.contains("recommendation of 4"),
            "rendered: {rendered}"
        );
    }

    // -----------------------------------------------------------------
    // MaxTotalWorkersAboveRamBudget
    // -----------------------------------------------------------------

    #[test]
    fn warns_when_max_total_workers_exceeds_recommendation() {
        let cfg = ServerConfig {
            max_total_workers: Some(64),
            ..Default::default()
        };
        let facts = apple_silicon_64gb();
        let recommended = super::recommend_max_total_workers(&facts);
        let result = validate(&cfg, &facts);
        assert_eq!(
            result.warnings,
            vec![ConfigWarning::MaxTotalWorkersAboveRamBudget {
                configured: 64,
                recommended,
            }],
        );
    }

    #[test]
    fn silent_when_max_total_workers_equals_recommendation() {
        let facts = apple_silicon_64gb();
        let recommended = super::recommend_max_total_workers(&facts);
        let cfg = ServerConfig {
            max_total_workers: Some(recommended),
            ..Default::default()
        };
        let result = validate(&cfg, &facts);
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn silent_when_max_total_workers_below_recommendation() {
        let cfg = ServerConfig {
            max_total_workers: Some(2),
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn silent_when_max_total_workers_is_none() {
        let cfg = ServerConfig {
            max_total_workers: None,
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn max_total_workers_warning_display_includes_both_values() {
        let warning = ConfigWarning::MaxTotalWorkersAboveRamBudget {
            configured: 64,
            recommended: 10,
        };
        let rendered = format!("{warning}");
        assert!(
            rendered.contains("max_total_workers=64"),
            "rendered: {rendered}"
        );
        assert!(
            rendered.contains("recommendation of 10"),
            "rendered: {rendered}"
        );
    }

    /// Display formatting must mention both values so the operator
    /// can act without consulting external docs.
    #[test]
    fn warning_display_includes_configured_and_recommended_values() {
        let warning = ConfigWarning::GpuThreadPoolSizeAboveOneOnCpu {
            configured: 4,
            recommended: 1,
        };
        let rendered = format!("{warning}");
        assert!(
            rendered.contains("gpu_thread_pool_size=4"),
            "rendered: {rendered}"
        );
        assert!(
            rendered.contains("recommended value is 1"),
            "rendered: {rendered}"
        );
    }

    // -----------------------------------------------------------------
    // ForceCpuFalseOnNonFunctionalGpu
    // -----------------------------------------------------------------

    #[test]
    fn warns_when_force_cpu_false_on_apple_silicon() {
        let cfg = ServerConfig {
            force_cpu: Some(false),
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert_eq!(result.warnings.len(), 1);
        assert!(matches!(
            result.warnings[0],
            ConfigWarning::ForceCpuFalseOnNonFunctionalGpu { .. }
        ));
    }

    /// `Some(false)` on a functional CUDA host: the recommendation
    /// agrees (false), so no contradiction.
    #[test]
    fn silent_when_force_cpu_false_on_cuda() {
        let cfg = ServerConfig {
            force_cpu: Some(false),
            ..Default::default()
        };
        let result = validate(&cfg, &linux_cuda_24gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// `Some(true)` on Apple Silicon: matches the recommendation;
    /// silent.
    #[test]
    fn silent_when_force_cpu_true_on_apple_silicon() {
        let cfg = ServerConfig {
            force_cpu: Some(true),
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// `Some(true)` on CUDA: operator asserts CPU even though GPU
    /// is functional. Legitimate testing affordance — silent. The
    /// symmetric "operator over-conservative" case from the design
    /// doc.
    #[test]
    fn silent_when_force_cpu_true_on_cuda_legitimate_testing_use() {
        let cfg = ServerConfig {
            force_cpu: Some(true),
            ..Default::default()
        };
        let result = validate(&cfg, &linux_cuda_24gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    /// `None` defers to recommendation; never contradicts.
    #[test]
    fn silent_when_force_cpu_is_none() {
        let cfg = ServerConfig {
            force_cpu: None,
            ..Default::default()
        };
        let result = validate(&cfg, &apple_silicon_64gb());
        assert!(
            result.warnings.is_empty(),
            "warnings: {:?}",
            result.warnings
        );
    }

    #[test]
    fn force_cpu_warning_display_includes_gpu_summary() {
        let warning = ConfigWarning::ForceCpuFalseOnNonFunctionalGpu {
            gpu: GpuPresence::AppleMps {
                functional_for_batchalign: false,
                reason_excluded: None,
            },
        };
        let rendered = format!("{warning}");
        assert!(rendered.contains("force_cpu=false"), "rendered: {rendered}");
        assert!(rendered.contains("AppleMps"), "rendered: {rendered}");
    }

    // -----------------------------------------------------------------
    // Multi-variant composition
    // -----------------------------------------------------------------

    /// An operator who sets several overrides above their respective
    /// recommendations gets one warning per knob. The validator runs
    /// each rule independently — there's no implicit shadowing or
    /// short-circuit. Order is the order the rules execute in
    /// [`validate`]; if a future refactor reorders them, this test
    /// will catch it (so the assertion uses set-membership rather
    /// than exact slice equality where order isn't load-bearing).
    #[test]
    fn multiple_above_recommendation_overrides_each_fire_independently() {
        let cfg = ServerConfig {
            gpu_thread_pool_size: Some(8),
            max_concurrent_jobs: Some(99),
            max_total_workers: Some(64),
            ..Default::default()
        };
        let facts = apple_silicon_64gb();
        let result = validate(&cfg, &facts);

        assert_eq!(
            result.warnings.len(),
            3,
            "expected three independent warnings; got: {:?}",
            result.warnings
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, ConfigWarning::GpuThreadPoolSizeAboveOneOnCpu { .. })),
            "missing GpuThreadPoolSizeAboveOneOnCpu in {:?}",
            result.warnings
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, ConfigWarning::MaxConcurrentJobsAboveRamBudget { .. })),
            "missing MaxConcurrentJobsAboveRamBudget in {:?}",
            result.warnings
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, ConfigWarning::MaxTotalWorkersAboveRamBudget { .. })),
            "missing MaxTotalWorkersAboveRamBudget in {:?}",
            result.warnings
        );
        // `max_concurrent_jobs: 99` on a 64 GB host also trips the
        // deterministic-OOM error (99 × 16 GB peak = 1584 GB).
        // Compositionally: the same configured value can fire BOTH
        // a warning (above the host-tier recommendation) and an
        // error (would OOM) — they're independent rules over the
        // same input.
        assert!(
            result.errors.iter().any(|e| matches!(
                e,
                ConfigError::MaxConcurrentJobsWouldDeterministicallyOom { .. }
            )),
            "missing MaxConcurrentJobsWouldDeterministicallyOom in {:?}",
            result.errors
        );
    }

    /// A clean ServerConfig::default() against a real-shape host
    /// produces zero warnings and zero errors. This is the contract
    /// that lets `ServerConfig::default()` deploy on any host without
    /// pre-startup churn — recommendations match the absence of
    /// overrides by construction.
    #[test]
    fn default_config_against_any_host_produces_no_findings() {
        let cfg = ServerConfig::default();
        for facts in [apple_silicon_64gb(), linux_cuda_24gb()] {
            let result = validate(&cfg, &facts);
            assert!(
                result.warnings.is_empty(),
                "default config produced warnings on {:?}: {:?}",
                facts.os,
                result.warnings
            );
            assert!(!result.has_errors());
        }
    }

    // -----------------------------------------------------------------
    // MaxConcurrentJobsWouldDeterministicallyOom (the first ConfigError)
    // -----------------------------------------------------------------

    /// Apple Silicon at 64 GB RAM. Worst-case per-job peak = 16 GB.
    /// `4 * 16 = 64` exactly equals ram_total — fits. `5 * 16 = 80`
    /// exceeds; that's the OOM error.
    #[test]
    fn errors_when_max_concurrent_jobs_times_peak_exceeds_ram() {
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(5),
            ..Default::default()
        };
        let facts = apple_silicon_64gb();
        let result = validate(&cfg, &facts);
        assert!(result.has_errors(), "warnings: {:?}", result.warnings);
        assert!(matches!(
            result.errors[0],
            ConfigError::MaxConcurrentJobsWouldDeterministicallyOom { .. }
        ));
    }

    /// Exact-fit (`configured * peak == ram_total`) is allowed; the
    /// check is strict greater-than. Pins the boundary explicitly so
    /// a future formula tweak that flips strictness is caught.
    #[test]
    fn silent_when_max_concurrent_jobs_times_peak_exactly_fits_ram() {
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(4), // 4 * 16 GB = 64 GB exactly
            ..Default::default()
        };
        let facts = apple_silicon_64gb();
        let result = validate(&cfg, &facts);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    /// `None` (no operator override) never triggers the OOM error.
    /// The recommendation is by construction safe — this is the
    /// "remove the override and the daemon will boot" escape
    /// hatch named in the error message.
    #[test]
    fn silent_when_max_concurrent_jobs_is_none_even_on_constrained_host() {
        let cfg = ServerConfig {
            max_concurrent_jobs: None,
            ..Default::default()
        };
        let facts = apple_silicon_64gb();
        let result = validate(&cfg, &facts);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    /// On a CUDA host with 128 GB RAM, even configured = 8
    /// (matching recommendation) fits: 8 × 16 = 128 = ram_total.
    /// The error stays silent for the recommendation.
    #[test]
    fn silent_when_recommendation_value_fits() {
        let facts = linux_cuda_24gb(); // 128 GB RAM in the fixture
        let recommended = super::recommend_max_concurrent_jobs(&facts);
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(recommended),
            ..Default::default()
        };
        let result = validate(&cfg, &facts);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    /// Display message must include all three carrier values plus
    /// the actionable remediation (drop the field or reduce it).
    #[test]
    fn oom_error_display_includes_configured_ram_and_peak() {
        let err = ConfigError::MaxConcurrentJobsWouldDeterministicallyOom {
            configured: 8,
            ram_total_mb: 32_000,
            per_job_peak_mb: 16_000,
        };
        let rendered = format!("{err}");
        assert!(
            rendered.contains("max_concurrent_jobs=8"),
            "rendered: {rendered}"
        );
        assert!(rendered.contains("32000"), "rendered: {rendered}");
        assert!(rendered.contains("16000"), "rendered: {rendered}");
        assert!(
            rendered.contains("Drop `max_concurrent_jobs`"),
            "rendered: {rendered}"
        );
    }

    /// `ConfigValidation::has_errors()` reflects the new variant.
    /// Empty validation still reports no errors.
    #[test]
    fn has_errors_distinguishes_empty_from_populated() {
        let empty = ConfigValidation::default();
        assert!(!empty.has_errors());

        let populated = ConfigValidation {
            warnings: Vec::new(),
            errors: vec![ConfigError::MaxConcurrentJobsWouldDeterministicallyOom {
                configured: 8,
                ram_total_mb: 32_000,
                per_job_peak_mb: 16_000,
            }],
        };
        assert!(populated.has_errors());
    }

    // ---------------------------------------------------------------
    // 16 GB laptop / GHA-runner UX contract.
    //
    // The 2026-05-08 rearch ("memory-gate-landed", "idle-eviction-pre-
    // pass-landed", "cpu-loadavg-gate-landed", "cap-at-4-retired") was
    // supposed to make memory gating dynamic — observe actual RSS,
    // gate at admission, evict under pressure. The runtime gates land
    // that contract, but the boot-time host-facts validator was left
    // command-agnostic: it compares `max_concurrent_jobs × 16000 MB`
    // (a hardcoded GPU-worker peak) against `ram_total_mb`, regardless
    // of which commands the operator will actually run.
    //
    // On a 16 GB laptop running a default `uv tool install batchalign3`
    // for morphotag, that gate refuses startup: `1 × 16000 > 15989`.
    // The user sees no signal that `--sequential` would let them run
    // the same job locally. Tests below pin both halves of the fix:
    //
    // 1. The default 16 GB laptop must accept startup (no operator
    //    `max_concurrent_jobs` override); the recommendation by
    //    construction must be safe.
    // 2. Whenever the gate DOES refuse startup (legitimate
    //    over-subscription on any tier), the error must point the
    //    operator at the `--sequential` workaround.
    // ---------------------------------------------------------------

    use super::super::test_helpers::laptop_16gb;

    #[test]
    fn laptop_16gb_with_default_config_accepts_startup() {
        let cfg = ServerConfig::default();
        let facts = laptop_16gb();
        let result = validate(&cfg, &facts);
        assert!(
            result.errors.is_empty(),
            "default startup on a 16 GB laptop must not be refused; \
             the host-facts gate has to be capability-aware so a \
             non-GPU install isn't blocked by hypothetical GPU peak. \
             errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn laptop_16gb_with_explicit_max_concurrent_one_accepts_startup() {
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(1),
            ..Default::default()
        };
        let facts = laptop_16gb();
        let result = validate(&cfg, &facts);
        assert!(
            result.errors.is_empty(),
            "max_concurrent_jobs=1 on a 16 GB laptop must be accepted \
             — the runtime gates are responsible for refusing actual \
             OOM via observation, not the boot gate. errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn laptop_16gb_with_oversubscribed_max_concurrent_jobs_still_refuses() {
        // Legitimate over-subscription: even with a generous worst-case,
        // 8 jobs cannot fit on a 16 GB laptop. The gate must still catch
        // this — relaxing the laptop case must not turn off the gate
        // entirely.
        let cfg = ServerConfig {
            max_concurrent_jobs: Some(8),
            ..Default::default()
        };
        let facts = laptop_16gb();
        let result = validate(&cfg, &facts);
        assert!(
            result.has_errors(),
            "max_concurrent_jobs=8 on a 16 GB laptop is genuinely \
             unsafe and must still be refused"
        );
    }

    #[test]
    fn deterministically_oom_error_message_suggests_sequential_workaround() {
        // Operator UX: when the gate refuses startup for resource
        // reasons, the error must name the `--sequential` flag as the
        // recovery path. A user who hits this error needs to be told
        // they can still run a single job locally without the daemon's
        // memory gate.
        let err = ConfigError::MaxConcurrentJobsWouldDeterministicallyOom {
            configured: 8,
            ram_total_mb: 32_000,
            per_job_peak_mb: 16_000,
        };
        let rendered = format!("{err}");
        assert!(
            rendered.contains("--sequential"),
            "resource-refusal error must point at --sequential. rendered: {rendered}"
        );
    }
}
