//! Runtime constants — loaded from `runtime_constants.toml` at compile time.
//!
//! Command-to-task mapping, memory budgets, and command classification.
//! The TOML file is the single source of truth shared with Python.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::api::MemoryMb;

pub use batchalign_types::memory::{
    MemoryTier, MemoryTierKind, estimate_per_worker_peak_mb,
    estimate_per_worker_peak_mb_with_profile,
};

/// Raw TOML content, embedded at compile time.
const TOML_SRC: &str = include_str!("../../../../../python/batchalign/runtime_constants.toml");

/// Parsed TOML structure.
// The gpu_heavy_commands field is retained for TOML structural validation.
// The Rust accessor was deleted in Phase β Task 4; classification now
// comes from COMMAND_SPECS in batchalign-types.
#[derive(Deserialize)]
#[allow(dead_code)]
struct RuntimeConstants {
    cmd2task: HashMap<String, String>,
    worker_caps: WorkerCaps,
    memory: MemoryConstants,
    gpu_heavy_commands: GpuHeavy,
    command_base_mb: CommandBaseMb,
    known_engine_keys: KnownEngineKeys,
}

#[derive(Deserialize)]
struct WorkerCaps {
    max_gpu_workers: usize,
    max_process_workers: usize,
    max_thread_workers: usize,
}

#[derive(Deserialize)]
struct MemoryConstants {
    default_base_mb: u64,
    mb_per_file_mb: u64,
    loading_overhead: f64,
}

// The gpu_heavy_commands TOML section is deserialized to validate the TOML at startup,
// but the Rust accessor is gone — classification now comes from COMMAND_SPECS in
// batchalign-types. Python still reads this section directly at import time.
// Task 5 codegen will project it from COMMAND_SPECS so the TOML stays in sync.
#[derive(Deserialize)]
#[allow(dead_code)]
struct GpuHeavy {
    commands: Vec<String>,
}

// Note: [process_commands] section is intentionally not deserialized here.
// Python reads it from runtime_constants.toml directly for GIL-aware dispatch
// classification. Rust selects process vs. threaded budgets via
// `is_free_threaded_runtime()` — see `command_execution_budget_mb()`.

#[derive(Deserialize)]
struct CommandBaseMb {
    process: HashMap<String, u64>,
    threaded: HashMap<String, u64>,
}

#[derive(Deserialize)]
struct KnownEngineKeys {
    keys: Vec<String>,
}

// Compile-time-constant embedded TOML — structurally validated by the test suite.
#[allow(clippy::expect_used)]
static CONSTANTS: LazyLock<RuntimeConstants> =
    LazyLock::new(|| toml::from_str(TOML_SRC).expect("runtime_constants.toml must be valid TOML"));

/// Whether the server is running in a free-threaded Python environment.
///
/// Detected once at process startup from the `PYTHON_GIL` environment variable:
/// `PYTHON_GIL=0` means CPython's GIL is disabled (Python 3.14t+). This is the
/// same variable that controls Python worker startup and is inherited by the Rust
/// server binary when launched from a free-threaded context.
///
/// When `true`:
/// - `command_execution_budget_mb()` uses the `threaded` (lower) table
/// - Stanza workers use concurrent serving (shared model via `ThreadPoolExecutor`)
///
/// When `false` (default):
/// - `command_execution_budget_mb()` uses the `process` (higher, conservative) table
/// - Stanza workers use sequential exclusive checkout
static FREE_THREADED_RUNTIME: LazyLock<bool> =
    LazyLock::new(|| std::env::var("PYTHON_GIL").as_deref() == Ok("0"));

/// Whether the current Rust server process is running alongside free-threaded Python workers.
///
/// Set by `PYTHON_GIL=0` in the environment. Used to select memory budgets and
/// dispatch routing for Stanza (CPU-bound) workers.
pub fn is_free_threaded_runtime() -> bool {
    *FREE_THREADED_RUNTIME
}

/// Command name -> pipeline task string (e.g. "align" -> "fa").
pub fn cmd2task() -> HashMap<&'static str, &'static str> {
    CONSTANTS
        .cmd2task
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

/// Known engine-override keys (passed via --engine-overrides).
pub fn known_engine_keys() -> &'static [String] {
    &CONSTANTS.known_engine_keys.keys
}

/// Hard cap on concurrent GPU-bound workers (transcribe, align, benchmark).
pub fn max_gpu_workers() -> usize {
    CONSTANTS.worker_caps.max_gpu_workers
}

/// Hard cap on concurrent process-isolated workers (non-free-threaded Python).
pub fn max_process_workers() -> usize {
    CONSTANTS.worker_caps.max_process_workers
}

/// Hard cap on concurrent thread workers (free-threaded Python 3.14t+).
pub fn max_thread_workers() -> usize {
    CONSTANTS.worker_caps.max_thread_workers
}

/// Per-command base memory (MB) — non-free-threaded (process workers).
pub fn command_base_mb_process() -> HashMap<&'static str, MemoryMb> {
    CONSTANTS
        .command_base_mb
        .process
        .iter()
        .map(|(k, &v)| (k.as_str(), MemoryMb(v)))
        .collect()
}

/// Per-command base memory (MB) — free-threaded (thread workers, shared models).
pub fn command_base_mb_threaded() -> HashMap<&'static str, MemoryMb> {
    CONSTANTS
        .command_base_mb
        .threaded
        .iter()
        .map(|(k, &v)| (k.as_str(), MemoryMb(v)))
        .collect()
}

/// Fallback per-worker memory budget (MB) when a command is not listed.
pub fn default_base_mb() -> MemoryMb {
    MemoryMb(CONSTANTS.memory.default_base_mb)
}

/// Conservative per-command execution reservation (MB) used for job-level host
/// memory planning.
///
/// Selects the `process` (conservative) memory table on GIL-enabled Python and
/// the `threaded` (shared-model) table when `PYTHON_GIL=0` is set. The process
/// table accounts for each concurrent Stanza worker holding a full private copy
/// of the model; the threaded table accounts for workers sharing one model via
/// OS threads on free-threaded Python 3.14t.
///
/// The loading-overhead factor applies to both tables — requests carry transient
/// tensor buffers even when the base model weights are shared.
pub fn command_execution_budget_mb(command: &str) -> MemoryMb {
    let table = if is_free_threaded_runtime() {
        &CONSTANTS.command_base_mb.threaded
    } else {
        &CONSTANTS.command_base_mb.process
    };
    let base = table
        .get(command)
        .copied()
        .unwrap_or(CONSTANTS.memory.default_base_mb);
    MemoryMb((base as f64 * CONSTANTS.memory.loading_overhead) as u64)
}

/// Additional memory budget (MB) allocated per file queued to a worker.
pub fn mb_per_file_mb() -> MemoryMb {
    MemoryMb(CONSTANTS.memory.mb_per_file_mb)
}

/// Multiplier applied to the static memory budget to account for transient
/// allocation spikes during model loading.
pub fn loading_overhead() -> f64 {
    CONSTANTS.memory.loading_overhead
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_parses_successfully() {
        // Force LazyLock initialization — panics if TOML is malformed.
        let _ = cmd2task();
    }

    #[test]
    fn cmd2task_contains_core_commands() {
        let map = cmd2task();
        assert_eq!(map["align"], "fa");
        assert_eq!(map["morphotag"], "morphosyntax");
        assert_eq!(map["transcribe"], "asr");
    }

    #[test]
    fn worker_caps_are_positive() {
        assert!(max_gpu_workers() > 0);
        assert!(max_process_workers() > 0);
        assert!(max_thread_workers() > 0);
    }

    #[test]
    fn memory_constants_are_sane() {
        assert!(default_base_mb().0 > 0);
        assert!(mb_per_file_mb().0 > 0);
        assert!(loading_overhead() > 1.0);
        // Per spec Principle 1, MemoryTier is the canonical source for
        // per-tier per-profile startup envelopes. Pin the Large-tier ordering.
        let tier = MemoryTier::from_total_mb(64_000);
        assert!(tier.gpu_startup_mb.0 > tier.stanza_startup_mb.0);
        assert!(tier.stanza_startup_mb.0 > tier.io_startup_mb.0);
        // Budget uses process table on GIL=1 (default in test env).
        assert!(command_execution_budget_mb("align").0 >= command_base_mb_process()["align"].0);
    }

    #[test]
    fn gpu_heavy_commands_exist_in_registry() {
        use batchalign_types::command_spec::COMMAND_SPECS;
        use batchalign_types::worker_profile::WorkerProfile;
        // Registry must have at least one GPU command (align, transcribe, benchmark, transcribe_s).
        assert!(
            COMMAND_SPECS
                .iter()
                .any(|s| s.profile == WorkerProfile::Gpu)
        );
    }

    #[test]
    fn command_base_mb_has_all_commands() {
        let proc = command_base_mb_process();
        let thread = command_base_mb_threaded();
        // Both maps should have the same keys
        let mut proc_keys: Vec<_> = proc.keys().collect();
        let mut thread_keys: Vec<_> = thread.keys().collect();
        proc_keys.sort();
        thread_keys.sort();
        assert_eq!(proc_keys, thread_keys);
    }

    // ---- MemoryTier ----

    #[test]
    fn tier_16gb_laptop() {
        let tier = MemoryTier::from_total_mb(16_000);
        assert_eq!(tier.kind, MemoryTierKind::Small);
        assert_eq!(tier.headroom_mb.0, 2_000);
        assert_eq!(tier.stanza_startup_mb.0, 3_000);
        assert_eq!(tier.gpu_startup_mb.0, 6_000);
        assert_eq!(tier.io_startup_mb.0, 2_000);
        assert_eq!(tier.max_suggested_workers, 1);
    }

    #[test]
    fn tier_32gb_workstation() {
        let tier = MemoryTier::from_total_mb(32_000);
        assert_eq!(tier.kind, MemoryTierKind::Medium);
        assert_eq!(tier.headroom_mb.0, 4_000);
        assert_eq!(tier.stanza_startup_mb.0, 6_000);
        // LazyProfile: GPU startup is just process overhead (3 GB), not full model.
        assert_eq!(tier.gpu_startup_mb.0, 3_000);
        // Max 1 worker to prevent OOM on 32 GB.
        assert_eq!(tier.max_suggested_workers, 1);
    }

    #[test]
    fn tier_64gb_fleet() {
        let tier = MemoryTier::from_total_mb(64_000);
        assert_eq!(tier.kind, MemoryTierKind::Large);
        assert_eq!(tier.headroom_mb.0, 8_000);
        assert_eq!(tier.stanza_startup_mb.0, 12_000);
        assert_eq!(tier.gpu_startup_mb.0, 16_000);
        assert_eq!(tier.max_suggested_workers, 4);
    }

    #[test]
    fn tier_256gb_server() {
        let tier = MemoryTier::from_total_mb(256_000);
        assert_eq!(tier.kind, MemoryTierKind::Fleet);
        assert_eq!(tier.headroom_mb.0, 8_000);
        assert_eq!(tier.stanza_startup_mb.0, 12_000);
        assert_eq!(tier.max_suggested_workers, 8);
    }

    #[test]
    fn small_machine_stanza_probe_passes_gate() {
        // Simulate: 16 GB total, macOS reports ~9 GB available
        let tier = MemoryTier::from_total_mb(16_000);
        let available = 9_000u64;
        let requested = tier.stanza_startup_mb.0;
        let reserve = tier.headroom_mb.0;
        // Gate formula: available - pending - requested >= reserve
        assert!(
            available.saturating_sub(requested) >= reserve,
            "Stanza probe must pass on 16 GB: {available} - {requested} = {} >= {reserve}",
            available - requested
        );
    }

    #[test]
    fn small_machine_gpu_and_stanza_concurrent_blocked() {
        // Two heavy workers at once should NOT fit on 16 GB
        let tier = MemoryTier::from_total_mb(16_000);
        let available = 9_000u64;
        let remaining_after_gpu = available.saturating_sub(tier.gpu_startup_mb.0);
        // After GPU reserved, Stanza should not fit within headroom
        assert!(
            remaining_after_gpu.saturating_sub(tier.stanza_startup_mb.0) < tier.headroom_mb.0,
            "Concurrent GPU+Stanza must NOT fit on 16 GB"
        );
    }

    #[test]
    fn tier_detect_returns_valid_tier() {
        let tier = MemoryTier::detect();
        assert!(tier.total_mb > 0);
        assert!(tier.headroom_mb.0 > 0);
        assert!(tier.gpu_startup_mb.0 > tier.stanza_startup_mb.0);
        assert!(tier.stanza_startup_mb.0 > tier.io_startup_mb.0);
    }

    #[test]
    fn tier_boundary_24gb_is_medium() {
        assert_eq!(
            MemoryTier::from_total_mb(24_000).kind,
            MemoryTierKind::Medium
        );
        assert_eq!(
            MemoryTier::from_total_mb(23_999).kind,
            MemoryTierKind::Small
        );
    }

    #[test]
    fn tier_boundary_48gb_is_large() {
        assert_eq!(
            MemoryTier::from_total_mb(48_000).kind,
            MemoryTierKind::Large
        );
        assert_eq!(
            MemoryTier::from_total_mb(47_999).kind,
            MemoryTierKind::Medium
        );
    }

    #[test]
    fn tier_boundary_128gb_is_fleet() {
        assert_eq!(
            MemoryTier::from_total_mb(128_000).kind,
            MemoryTierKind::Fleet
        );
        assert_eq!(
            MemoryTier::from_total_mb(127_999).kind,
            MemoryTierKind::Large
        );
    }

    // -------------------------------------------------------------
    // tier_aware_command_execution_budget_mb — Layer 3 fix.
    //
    // The fleet-conservative `command_execution_budget_mb` returns
    // 12000 MB for morphotag (8000 base × 1.5 loading overhead).
    // On a 16 GB laptop that's 75% of physical RAM — too big a
    // single-job envelope, the host_memory coordinator refuses
    // worker spawn before any actual work happens. The tier-aware
    // variant clamps the worst-case to the tier's per-profile
    // startup reservation so small hosts get realistic envelopes.
    // -------------------------------------------------------------

    #[test]
    fn tier_aware_budget_clamps_morphotag_to_stanza_startup_on_small() {
        use batchalign_types::api::ReleasedCommand;
        use batchalign_types::command_spec::command_spec_for;
        use batchalign_types::memory::estimate_per_worker_peak_mb_with_profile;
        let tier = MemoryTier::from_total_mb(16_000);
        assert_eq!(tier.kind, MemoryTierKind::Small);
        let spec = command_spec_for(ReleasedCommand::Morphotag);
        let worst_case = batchalign_types::api::MemoryMb(
            (spec.base_mb_for_runtime(false).0 as f64 * spec.loading_overhead.0) as u64,
        );
        let budget = estimate_per_worker_peak_mb_with_profile(worst_case, spec.profile, &tier);
        assert!(
            budget.0 <= tier.stanza_startup_mb.0,
            "Small-tier morphotag budget must clamp to stanza_startup_mb \
             ({} MB), got {} MB",
            tier.stanza_startup_mb.0,
            budget.0
        );
        assert!(
            budget.0 < 12_000,
            "Small-tier morphotag budget must be smaller than the \
             fleet worst-case (12000 MB), got {} MB",
            budget.0
        );
    }

    #[test]
    fn tier_aware_budget_keeps_fleet_morphotag_at_full_worst_case() {
        use batchalign_types::api::ReleasedCommand;
        use batchalign_types::command_spec::command_spec_for;
        use batchalign_types::memory::estimate_per_worker_peak_mb_with_profile;
        let tier = MemoryTier::from_total_mb(256_000);
        assert_eq!(tier.kind, MemoryTierKind::Fleet);
        let spec = command_spec_for(ReleasedCommand::Morphotag);
        let worst_case = batchalign_types::api::MemoryMb(
            (spec.base_mb_for_runtime(false).0 as f64 * spec.loading_overhead.0) as u64,
        );
        let budget = estimate_per_worker_peak_mb_with_profile(worst_case, spec.profile, &tier);
        assert_eq!(
            budget,
            command_execution_budget_mb("morphotag"),
            "Fleet-tier morphotag must equal the fleet worst-case; the \
             clamp must not weaken fleet behavior"
        );
    }

    #[test]
    fn tier_aware_budget_uses_gpu_envelope_for_align_on_small() {
        use batchalign_types::api::ReleasedCommand;
        use batchalign_types::command_spec::command_spec_for;
        use batchalign_types::memory::estimate_per_worker_peak_mb_with_profile;
        let tier = MemoryTier::from_total_mb(16_000);
        let spec = command_spec_for(ReleasedCommand::Align);
        let worst_case = batchalign_types::api::MemoryMb(
            (spec.base_mb_for_runtime(false).0 as f64 * spec.loading_overhead.0) as u64,
        );
        let budget = estimate_per_worker_peak_mb_with_profile(worst_case, spec.profile, &tier);
        assert!(
            budget.0 <= tier.gpu_startup_mb.0,
            "Small-tier align budget must clamp to gpu_startup_mb \
             ({} MB), got {} MB",
            tier.gpu_startup_mb.0,
            budget.0
        );
    }

    /// End-to-end check: with the tier-aware budget, a 16 GB host
    /// admits 1-worker morphotag through `plan_job_reservation`,
    /// matching the realistic CI scenario (13902 MB available after
    /// kernel/agent overhead, 2048 MB reserve floor).
    #[test]
    fn tier_aware_budget_lets_16gb_host_admit_1_worker_morphotag() {
        use batchalign_types::api::ReleasedCommand;
        use batchalign_types::command_spec::command_spec_for;
        use batchalign_types::memory::estimate_per_worker_peak_mb_with_profile;
        let tier = MemoryTier::from_total_mb(16_000);
        let spec = command_spec_for(ReleasedCommand::Morphotag);
        let worst_case = batchalign_types::api::MemoryMb(
            (spec.base_mb_for_runtime(false).0 as f64 * spec.loading_overhead.0) as u64,
        );
        let budget = estimate_per_worker_peak_mb_with_profile(worst_case, spec.profile, &tier);
        // plan_job_reservation lives in host_memory.rs; reproduce its
        // math here so the test is self-contained without
        // cross-module wiring. The bug we're fixing is upstream of
        // plan_job_reservation — the BUDGET it receives.
        let available_mb = 13_902u64;
        let reserve_mb = 2_048u64;
        let pending_reserved_mb = 0u64;
        let projected_after = available_mb
            .saturating_sub(pending_reserved_mb)
            .saturating_sub(budget.0);
        assert!(
            projected_after >= reserve_mb,
            "Tier-aware budget must let a 16 GB host admit 1-worker \
             morphotag: budget={budget_mb} MB, available={available_mb} \
             MB, projected_after={projected_after} MB, reserve={reserve_mb} MB",
            budget_mb = budget.0,
        );
    }

    /// Architectural invariant for spec Principle 1:
    /// `MemoryTier::{gpu,stanza,io}_startup_mb` is the SOLE canonical source of
    /// per-tier per-profile worker startup memory. No parallel constants,
    /// functions, or struct fields holding the same concept may exist anywhere
    /// else in the production code of the `batchalign` crate.
    ///
    /// This test is identifier-shape-anchored (not numeric-value-anchored)
    /// because the canonical Large-tier values (16000, 12000, 4000) appear
    /// in legitimate unrelated contexts (audio sample rates, byte buffers).
    ///
    /// Allowlisted definition sites:
    ///   - `crates/batchalign-types/src/memory.rs` — canonical `MemoryTier` struct
    ///     (moved from `crates/batchalign/src/types/runtime.rs` in Phase β Task 2).
    ///   - `crates/batchalign/src/types/config/server.rs` — operator-override
    ///     fields on `RuntimeOverridesConfig` (flow INTO `MemoryTier`, not parallel to it).
    ///
    /// The scan is scoped to `batchalign/src` so the canonical definition in
    /// `batchalign-types` is never in scope; only illicit parallel copies would appear.
    ///
    /// On regression the test fails with a message pointing at the spec.
    #[test]
    fn memory_tier_is_sole_source_for_per_profile_envelopes() {
        // (a) Pin the canonical Large-tier values. Any change here must be
        //     accompanied by a deliberate update of the spec.
        let tier = MemoryTier::from_total_mb(64_000);
        assert_eq!(tier.gpu_startup_mb.0, 16_000, "Large-tier GPU envelope");
        assert_eq!(
            tier.stanza_startup_mb.0, 12_000,
            "Large-tier Stanza envelope"
        );
        assert_eq!(tier.io_startup_mb.0, 4_000, "Large-tier IO envelope");

        // (b) Scan production sources in batchalign/src for parallel definitions.
        //
        // Resolve paths via a workspace-marker walk rather than a fixed
        // `../../` traversal — the latter breaks if the crate ever moves.
        // The marker is the workspace Cargo.toml containing `[workspace]`.
        let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let src_root = crate_root.join("src");
        let workspace_root = find_workspace_root(&crate_root)
            .expect("workspace root not found — Cargo.toml with [workspace] absent above CARGO_MANIFEST_DIR");

        // Definition-shape regex: matches top-level fn/const/static or `pub` struct
        // fields named *_startup_mb (gpu, stanza, or io). Field-access expressions
        // like `tier.gpu_startup_mb` and doc-comment mentions are NOT matched.
        let regex = r"^\s*(pub\s+)?(fn|const|static)\s+(gpu|stanza|io)_startup_mb\b|^\s*pub\s+(gpu|stanza|io)_startup_mb\s*:";

        // Excluded file: the operator-override config whose fields flow INTO MemoryTier.
        // The canonical MemoryTier struct is in batchalign-types/src/memory.rs and is
        // not under src_root, so no exclude needed for it.
        let excludes = ["--glob=!**/types/config/server.rs"];

        let output = std::process::Command::new("rg")
            .arg("--no-heading")
            .arg("--line-number")
            .arg("--multiline-dotall")
            .arg("-e")
            .arg(regex)
            .args(excludes)
            .arg(&src_root)
            .output()
            .expect("ripgrep must be installed and on PATH for this test");

        // ripgrep exits 1 when there are no matches — that is the expected state.
        // Exit 0 means matches were found, which is a regression.
        if output.status.success() {
            let matches = String::from_utf8_lossy(&output.stdout);
            panic!(
                "Architectural invariant violation (spec Principle 1):\n\
                 found parallel definition(s) of *_startup_mb inside batchalign/src.\n\
                 MemoryTier in batchalign-types/src/memory.rs is the SOLE canonical source.\n\
                 Operator overrides via RuntimeOverridesConfig are the only allowed channel.\n\
                 See docs/architecture/2026-05-10-tier-aware-memory-consolidation.md \
                 (Phase α, Principle 1).\n\n\
                 ripgrep matches:\n{matches}"
            );
        }

        // (c) Also catch reintroduction of the deleted TOML section name.
        //     The bare token `worker_startup_mb` was the key for the deleted
        //     [worker_startup_mb] table; it must not return.
        let toml_scan = std::process::Command::new("rg")
            .arg("--no-heading")
            .arg("--line-number")
            .arg("-e")
            .arg(r"\bworker_startup_mb\b")
            .arg(workspace_root.join("python/batchalign/runtime_constants.toml"))
            .output()
            .expect("ripgrep must be installed and on PATH for this test");

        if toml_scan.status.success() {
            let matches = String::from_utf8_lossy(&toml_scan.stdout);
            panic!(
                "Architectural invariant violation (spec Principle 1):\n\
                 the `[worker_startup_mb]` TOML section was deleted in Phase α \
                 and must not be reintroduced. MemoryTier is the canonical source.\n\n\
                 ripgrep matches:\n{matches}"
            );
        }
    }

    /// Walk up from `start` looking for the workspace `Cargo.toml`
    /// (the one containing `[workspace]`). Returns the directory.
    /// Used by the architectural-invariant test to anchor file
    /// scans without baked-in `../../` traversal.
    fn find_workspace_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
        let mut current = start;
        loop {
            let cargo_toml = current.join("Cargo.toml");
            if cargo_toml.is_file() {
                let contents = std::fs::read_to_string(&cargo_toml).ok()?;
                if contents.contains("[workspace]") {
                    return Some(current.to_path_buf());
                }
            }
            current = current.parent()?;
        }
    }
}
