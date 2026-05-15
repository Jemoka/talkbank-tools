//! Per-job file parallelism auto-tuning and media constants.

use crate::api::{NumWorkers, ReleasedCommand};
use crate::config::ServerConfig;
use crate::host_facts::EffectiveConfig;
use crate::runtime;

/// Known audio/video file extensions for media pre-validation.
/// Known audio/video file extensions for media resolution.
pub const KNOWN_MEDIA_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "mp4", "m4a", "flac", "ogg", "aac", "wma", "webm",
];

/// Compute the number of parallel file workers for a job.
///
/// Reads from the resolved [`EffectiveConfig`] (operator override merged
/// with host-facts recommendation) for the per-command worker cap, the
/// host-facts-derived GPU thread-pool cap, and the GPU-heavy-vs-CPU
/// category split. The legacy [`ServerConfig`] is consulted only for
/// `resolved_memory_tier()`, which honors the operator's
/// `memory_tier` override (used by the test affordance for
/// constrained-memory simulation on large hosts) — this knob has no
/// `EffectiveConfig` analog yet.
///
/// File-count clamping (`min(num_files)`) and CPU clamping
/// (`min(available_parallelism)`) stay at the dispatch site because
/// they're per-job quantities; host-quantities live in `EffectiveConfig`.
///
/// This function intentionally does **not** do host-memory math anymore.
/// It only applies file-count, operator-configured, CPU, and
/// per-category caps. Host-wide memory clamping now happens in the
/// coordinator-backed admission step so worker startup and job
/// execution share one memory model.
pub(in crate::runner) fn compute_job_workers(
    command: ReleasedCommand,
    num_files: usize,
    effective: &EffectiveConfig,
    config: &ServerConfig,
) -> NumWorkers {
    if num_files <= 1 {
        return NumWorkers(1);
    }

    let is_gpu_heavy = batchalign_types::command_spec::command_spec_for(command).profile
        == batchalign_types::worker_profile::WorkerProfile::Gpu;

    let by_cpu = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);
    let tier_cap = config.resolved_memory_tier().max_suggested_workers;

    // Apply per-category cap. GPU commands share one model process and
    // should not dispatch more in-process requests than the configured
    // GPU thread pool intends to serve. Auto-tuned jobs must also
    // respect the host tier's suggested worker ceiling so
    // medium-memory machines do not request more file parallelism than
    // startup reservations can support.
    let gpu_thread_pool_size = effective.gpu_thread_pool_size as usize;
    let category_cap = if is_gpu_heavy {
        runtime::max_gpu_workers().min(gpu_thread_pool_size)
    } else {
        runtime::max_thread_workers()
    }
    .min(tier_cap);

    // The per-command resolved cap from `EffectiveConfig` already
    // incorporates the operator's uniform override (`Some(n)` from
    // `ConfigOverrides::max_workers_per_job`) when set, falling
    // through to the per-command host-facts recommendation otherwise.
    let resolved_cap = effective.max_workers_per_job(&command) as usize;

    NumWorkers(
        num_files
            .min(by_cpu)
            .min(resolved_cap)
            .clamp(1, category_cap),
    )
}
