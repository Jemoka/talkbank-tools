//! Worker pool preparation for both direct and server execution hosts.
//!
//! This module owns the capability probing, warmup, and pool construction
//! logic that is shared between direct-mode CLI execution and the HTTP
//! server. It does NOT depend on axum, sqlx, or any server-specific crate.

use std::collections::BTreeMap;
use std::sync::Arc;

use tracing::info;

use crate::cache::UtteranceCache;
use crate::capability::{
    WorkerCapabilitySnapshot, resolve_worker_capability_snapshot, validate_infer_capability_gate,
};
use crate::commands::{released_command_definition, released_command_definitions};
use crate::config::ServerConfig;
use crate::error;
use crate::host_policy::HostExecutionPolicy;
use crate::runner::{ExecutionEngine, RunnerExecutionContext};
use crate::worker::InferTask;
use crate::worker::pool::{PoolConfig, WorkerPool};

// ---------------------------------------------------------------------------
// Prepared worker subsystem
// ---------------------------------------------------------------------------

/// Prepared worker subsystem that can be reused across multiple app instances.
///
/// Tests use this seam to amortize capability probing and model warmup while
/// still creating a fresh control plane and runtime-owned filesystem layout
/// for each isolated session.
#[derive(Clone)]
pub struct PreparedWorkers {
    pool: Arc<WorkerPool>,
    capabilities: Vec<String>,
    infer_tasks: Vec<InferTask>,
    engine_versions: BTreeMap<String, String>,
    test_echo_mode: bool,
}

/// One host-neutral execution runtime resolved from prepared workers.
pub(crate) struct ResolvedExecutionRuntime {
    pub capability_snapshot: WorkerCapabilitySnapshot,
    pub engine: ExecutionEngine,
}

/// A command + language pair to pre-warm at server startup.
///
/// Both fields are already validated at construction time so downstream
/// consumers do not need to re-parse or handle invalid values.
#[derive(Debug, Clone)]
pub struct WarmupTarget {
    /// Released command to warm (validated from config at construction).
    pub command: crate::api::ReleasedCommand,
    /// Language to warm the command for (validated from config at construction).
    pub lang: crate::api::WorkerLanguage,
}

impl PreparedWorkers {
    /// Resolve the latest capability snapshot, preferring live detected worker
    /// data over the startup placeholder snapshot when available.
    pub(crate) fn capability_snapshot(
        &self,
    ) -> Result<WorkerCapabilitySnapshot, error::ServerError> {
        resolve_worker_capability_snapshot(
            &self.capabilities,
            &self.infer_tasks,
            &self.engine_versions,
            self.test_echo_mode,
            self.pool.detected_capabilities(),
        )
    }

    /// Return the released command surface discovered during worker probing.
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// Return the infer-task set reported by the prepared worker subsystem.
    pub fn infer_tasks(&self) -> &[InferTask] {
        &self.infer_tasks
    }

    /// Return the latest infer-task view, preferring live worker detection
    /// over the startup placeholder snapshot when available.
    pub fn current_infer_tasks(&self) -> Result<Vec<InferTask>, error::ServerError> {
        Ok(self.capability_snapshot()?.infer_tasks)
    }

    /// Build one host-neutral execution runtime over this prepared worker set.
    pub(crate) fn resolve_execution_runtime(
        &self,
        cache: Arc<UtteranceCache>,
    ) -> Result<ResolvedExecutionRuntime, error::ServerError> {
        let capability_snapshot = self.capability_snapshot()?;
        let engine = ExecutionEngine::new(RunnerExecutionContext::new(
            self.pool.clone(),
            cache,
            capability_snapshot.infer_tasks.clone(),
            capability_snapshot.engine_versions.clone(),
            self.test_echo_mode,
        ));
        Ok(ResolvedExecutionRuntime {
            capability_snapshot,
            engine,
        })
    }

    /// Return a reference to the underlying worker pool.
    ///
    /// Exposed for server-only code that needs direct pool access (e.g.,
    /// building [`AppState`](crate::state::AppState)).
    pub fn pool(&self) -> &Arc<WorkerPool> {
        &self.pool
    }
}

// ---------------------------------------------------------------------------
// Worker probing and preparation
// ---------------------------------------------------------------------------

/// Probe, validate, and optionally warm a worker pool for reuse.
///
/// The returned [`PreparedWorkers`] value owns a live [`WorkerPool`] plus the
/// capability metadata derived from it. Callers can share that value across
/// multiple app instances to keep expensive model loads hot while still
/// rebuilding the server control plane and runtime-owned temp directories.
///
/// Warmup runs synchronously (all commands spawn concurrently within the call).
/// For non-blocking startup, use [`prepare_workers_background`] which returns
/// immediately after capability probing and spawns warmup in a background task.
pub async fn prepare_workers(
    config: &ServerConfig,
    pool_config: PoolConfig,
) -> Result<PreparedWorkers, error::ServerError> {
    let (prepared, targets) = probe_workers(config, pool_config, true).await?;

    if !targets.is_empty() {
        prepared.pool.warmup(&targets).await;
    }
    prepared.pool.mark_warmup_complete();

    Ok(prepared)
}

/// Like [`prepare_workers`] but warmup runs as a background `tokio::spawn`
/// task.  The HTTP server can bind its port immediately while models load.
///
/// The returned [`PreparedWorkers`] is ready for use — jobs that arrive
/// before warmup finishes will block on checkout until their required worker
/// spawns, which is correct (no duplicate spawns).
pub async fn prepare_workers_background(
    config: &ServerConfig,
    pool_config: PoolConfig,
) -> Result<PreparedWorkers, error::ServerError> {
    let (prepared, targets) = probe_workers(config, pool_config, true).await?;

    if !targets.is_empty() {
        prepared.pool.mark_warmup_started();
        let warmup_pool = prepared.pool.clone();
        tokio::spawn(async move {
            warmup_pool.warmup(&targets).await;
            warmup_pool.mark_warmup_complete();
            info!("Background warmup complete");
        });
    } else {
        prepared.pool.mark_warmup_complete();
    }

    Ok(prepared)
}

/// Probe and validate one worker pool for direct inline execution.
///
/// Unlike server preparation, this path intentionally skips registry discovery
/// and host-wide warmup so direct mode does not adopt detached daemon behavior.
pub async fn prepare_direct_workers(
    config: &ServerConfig,
    pool_config: PoolConfig,
) -> Result<PreparedWorkers, error::ServerError> {
    let (prepared, _targets) = probe_workers(config, pool_config, false).await?;
    prepared.pool.mark_warmup_complete();
    Ok(prepared)
}

/// Build the worker pool with optimistic capabilities (no Python probe).
///
/// Capabilities are detected lazily on the first real worker spawn, not at
/// server startup. This eliminates the 10-30 second startup delay and 2-3 GB
/// peak memory spike from the probe worker on small machines.
///
/// For test-echo mode, capabilities are synthesized from `cmd2task()`.
async fn probe_workers(
    config: &ServerConfig,
    pool_config: PoolConfig,
    discover_registry_workers: bool,
) -> Result<(PreparedWorkers, Vec<WarmupTarget>), error::ServerError> {
    let test_echo_mode = pool_config.test_echo;
    let host_policy = HostExecutionPolicy::from_server_config(config);
    let pool = Arc::new(WorkerPool::new(pool_config));
    pool.start_background_tasks();

    if discover_registry_workers {
        // Discover pre-started TCP workers from the registry file.
        let discovered = pool.discover_from_registry().await;
        if discovered > 0 {
            info!(discovered, "Pre-started TCP workers integrated into pool");
        }
    }

    // Optimistic capabilities: accept all released commands.
    // Real capabilities are detected lazily on first worker spawn.
    let (capabilities, infer_tasks, engine_versions) = if test_echo_mode {
        let all_tasks: Vec<InferTask> = released_command_definitions()
            .iter()
            .map(|definition| definition.descriptor.infer_task)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let caps = validate_infer_capability_gate(&all_tasks, &BTreeMap::new(), true)?;
        (caps, all_tasks, BTreeMap::new())
    } else if let Some(detected) = pool.detected_capabilities() {
        // TCP registry workers were discovered and capabilities probed.
        let caps = validate_infer_capability_gate(
            &detected.infer_tasks,
            &detected.engine_versions,
            false,
        )?;
        info!(
            capabilities = ?caps,
            infer_tasks = ?detected.infer_tasks,
            "Using capabilities detected from TCP registry workers"
        );
        (
            caps,
            detected.infer_tasks.clone(),
            detected.engine_versions.clone(),
        )
    } else {
        // Optimistic mode: advertise all released commands and their infer
        // tasks. Real capabilities are refined lazily when the first worker
        // spawns and reports its actual engine versions. This avoids a
        // 10-30 second startup delay from spawning a probe worker.
        let all_tasks: Vec<InferTask> = released_command_definitions()
            .iter()
            .map(|definition| definition.descriptor.infer_task)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let caps = optimistic_capabilities();
        info!(
            capabilities = ?caps,
            "Using optimistic capabilities (lazy detection on first worker spawn)"
        );
        (caps, all_tasks, BTreeMap::new())
    };

    // No warmup targets — workers spawn on demand.
    let warmup_cmds = config.resolved_warmup_commands();
    let targets = if warmup_cmds.is_empty() {
        // Skip warmup in production — workers spawn lazily.
        // Test-echo mode can still warmup if configured.
        Vec::new()
    } else {
        let default_lang = crate::api::WorkerLanguage::from(config.default_lang.clone());
        warmup_cmds
            .iter()
            .filter(|cmd| capabilities.contains(cmd))
            .filter_map(|cmd| {
                crate::api::ReleasedCommand::try_from(cmd.as_str())
                    .ok()
                    .and_then(|command| {
                        let definition = released_command_definition(command);
                        host_policy
                            .allows_command_warmup(definition.warmup_policy(), test_echo_mode)
                            .then(|| WarmupTarget {
                                command,
                                lang: default_lang.clone(),
                            })
                    })
            })
            .collect()
    };

    if targets.is_empty() {
        info!("Worker warmup disabled (lazy start)");
    } else {
        info!(commands = ?targets, "Warmup commands resolved");
    }

    Ok((
        PreparedWorkers {
            pool,
            capabilities,
            infer_tasks,
            engine_versions,
            test_echo_mode,
        },
        targets,
    ))
}

/// All released commands — used as the optimistic capability set before
/// the first real worker spawn confirms what's actually installed.
fn optimistic_capabilities() -> Vec<String> {
    released_command_definitions()
        .iter()
        .map(|definition| definition.descriptor.command.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
