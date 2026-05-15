//! Type definitions for the runner module's execution context, host bundles,
//! and orchestration traits.
//!
//! These are the shared data structures that thread through job dispatch:
//! - `RunnerExecutionContext` — worker pool + cache + engine metadata
//! - `ExecutionEngine` — thin wrapper that dispatches to the routing layer
//! - `DispatchHostContext` — read-only server/config context for dispatch
//! - `ServerExecutionHost` / `DirectExecutionHost` — host-owned bundles
//! - `QueuedJobOrchestrator` — host-owned re-queue policy trait

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::api::{JobId, NumWorkers, RevAiJobId, UnixTimestamp};
use crate::cache::UtteranceCache;
use crate::config::ServerConfig;
use crate::host_facts::EffectiveConfig;
use crate::host_memory::HostMemoryError;
use crate::runner::util::{RunnerEventSink, StoreRunnerEventSink};
use crate::store::{JobStore, RunnerJobSnapshot};
use crate::worker::InferTask;
use crate::worker::pool::WorkerPool;

/// Shared dependencies needed to build per-job runner tasks.
#[derive(Clone)]
pub(crate) struct RunnerExecutionContext {
    pub(super) pool: Arc<WorkerPool>,
    pub(super) cache: Arc<UtteranceCache>,
    pub(super) infer_tasks: Vec<InferTask>,
    pub(super) engine_versions: BTreeMap<String, String>,
    pub(super) test_echo_mode: bool,
}

impl RunnerExecutionContext {
    pub(crate) fn new(
        pool: Arc<WorkerPool>,
        cache: Arc<UtteranceCache>,
        infer_tasks: Vec<InferTask>,
        engine_versions: BTreeMap<String, String>,
        test_echo_mode: bool,
    ) -> Self {
        Self {
            pool,
            cache,
            infer_tasks,
            engine_versions,
            test_echo_mode,
        }
    }
}

/// Shared execution engine built on one resolved runtime context.
#[derive(Clone)]
pub(crate) struct ExecutionEngine {
    pub(super) context: RunnerExecutionContext,
}

impl ExecutionEngine {
    /// Build one shared execution engine over a resolved runtime context.
    pub(crate) fn new(context: RunnerExecutionContext) -> Self {
        Self { context }
    }

    pub(super) async fn dispatch_job(
        &self,
        request: JobDispatchRequest,
        host: &DispatchHostContext,
    ) -> Result<(), crate::error::ServerError> {
        super::routing::dispatch_job_with_execution_context(request, host, &self.context).await
    }

    /// Borrow the underlying worker pool for cancel-driven shutdown.
    /// The cancel pathway in `ServerBackend::cancel_job` calls
    /// `pool.shutdown_workers_for_job` after the store cancel completes;
    /// without this accessor, the backend has no way to reach the pool.
    pub(crate) fn pool(&self) -> &Arc<WorkerPool> {
        &self.context.pool
    }
}

/// Read-only host/runtime context consulted during shared command dispatch.
///
/// This keeps performance policy and media-resolution config explicit host
/// concerns without threading the full mutable `JobStore` through command code.
///
/// Holds an [`EffectiveConfig`] resolved once at construction from the host's
/// `ServerConfig` plus the live `HostFacts` snapshot. Per-job dispatch reads
/// from this resolved view rather than re-detecting host facts on every call,
/// which is the architectural seam the host-facts migration introduced (see
/// `talkbank/docs/investigations/2026-04-25-host-facts-architecture.md`).
#[derive(Clone)]
pub(crate) struct DispatchHostContext {
    store: Arc<JobStore>,
    config: Arc<ServerConfig>,
    effective_config: Arc<EffectiveConfig>,
    sink: Arc<dyn RunnerEventSink>,
}

impl DispatchHostContext {
    pub(crate) fn from_store(store: Arc<JobStore>) -> Self {
        let config = store.config().clone();
        // Snapshot host facts once at host construction; per-job
        // dispatch reads the resolved view rather than re-polling.
        // Runtime memory pressure is handled by the live memory-guard
        // poll (`worker::memory_guard`), not by re-detecting facts.
        let effective_config = Arc::new(EffectiveConfig::resolve_from_server_config(&config));
        Self {
            store: store.clone(),
            config: Arc::new(config),
            effective_config,
            sink: StoreRunnerEventSink::wrap(store),
        }
    }

    pub(crate) fn config(&self) -> &ServerConfig {
        self.config.as_ref()
    }

    /// Resolved per-host configuration view (operator overrides merged with
    /// host-facts recommendations).
    pub(crate) fn effective_config(&self) -> &EffectiveConfig {
        self.effective_config.as_ref()
    }

    pub(crate) fn sink(&self) -> &Arc<dyn RunnerEventSink> {
        &self.sink
    }

    pub(crate) fn media_mapping_root(
        &self,
        key: &str,
    ) -> Option<&batchalign_types::paths::ServerPath> {
        self.config
            .media_mappings
            .get(&batchalign_types::paths::MediaMappingKey::new(key))
    }

    pub(crate) fn media_roots(&self) -> &[batchalign_types::paths::ServerPath] {
        &self.config.media_roots
    }

    pub(crate) fn trace_store(&self) -> &crate::trace_store::TraceStore {
        self.store.trace_store()
    }
}

/// Shared server-owned host dependencies needed to build per-job runner tasks.
#[derive(Clone)]
pub(crate) struct ServerExecutionHost {
    pub(super) store: Arc<JobStore>,
    pub(super) engine: ExecutionEngine,
    pub(super) orchestrator: Arc<dyn QueuedJobOrchestrator>,
}

impl ServerExecutionHost {
    /// Build the server-owned host bundle around one execution engine.
    pub(crate) fn new(
        store: Arc<JobStore>,
        engine: ExecutionEngine,
        orchestrator: Arc<dyn QueuedJobOrchestrator>,
    ) -> Self {
        Self {
            store,
            engine,
            orchestrator,
        }
    }

    /// Access the shared job store for cross-module coordination (e.g.,
    /// Temporal activity cancel forwarding dispatched to the main runtime).
    pub(crate) fn store(&self) -> &Arc<JobStore> {
        &self.store
    }

    /// Access the underlying worker pool. Backends call this from
    /// `cancel_job` to invoke `pool.shutdown_workers_for_job`, which
    /// SIGTERMs every in-flight worker registered to the cancelled job
    /// (see `worker/pool/job_tracker.rs`).
    pub(crate) fn pool(&self) -> &Arc<WorkerPool> {
        self.engine.pool()
    }
}

/// Shared direct-execution host dependencies needed to run one inline job.
#[derive(Clone)]
pub(crate) struct DirectExecutionHost {
    pub(super) store: Arc<JobStore>,
    pub(super) engine: ExecutionEngine,
}

impl DirectExecutionHost {
    /// Build one direct-execution host bundle around one execution engine.
    pub(crate) fn new(store: Arc<JobStore>, engine: ExecutionEngine) -> Self {
        Self { store, engine }
    }
}

/// Execution-phase request handed from the host-owned runner wrapper into the
/// shared dispatch kernel.
pub(super) struct JobDispatchRequest {
    pub(super) job: Arc<RunnerJobSnapshot>,
    pub(super) file_list: Vec<crate::store::PendingJobFile>,
    pub(super) num_workers: NumWorkers,
    pub(super) rev_job_ids: Arc<HashMap<PathBuf, RevAiJobId>>,
}

/// Host-memory reservation failures separated from the rest of job execution.
pub(super) enum ExecutionReservationError {
    Capacity {
        requested_workers: NumWorkers,
        error: HostMemoryError,
    },
    Fatal(crate::error::ServerError),
}

/// Host-owned orchestration decision after a memory-gate rejection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MemoryGateRejectionDisposition {
    /// The host re-queued the job for a later eligibility deadline.
    Requeued { retry_at: UnixTimestamp },
}

/// Result of one host-owned job execution attempt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HostedJobRunOutcome {
    /// The job finished its current lifecycle attempt.
    Completed,
    /// The host deferred the job for a later eligibility deadline.
    Requeued { retry_at: UnixTimestamp },
}

/// Host-owned orchestration seam for queued job execution policy.
///
/// This keeps the shared runner from knowing how a specific server backend
/// persists re-queues, computes backoff, or wakes its scheduler when a job
/// cannot currently run.
#[async_trait]
pub(crate) trait QueuedJobOrchestrator: Send + Sync {
    /// Handle a host-memory rejection for one queued job.
    async fn handle_memory_gate_rejection(
        &self,
        sink: &Arc<dyn RunnerEventSink>,
        job_id: &JobId,
        requested_workers: NumWorkers,
        error: &HostMemoryError,
    ) -> Result<MemoryGateRejectionDisposition, crate::error::ServerError>;
}

/// Policy for handling memory-gate failures during job execution.
pub(super) enum MemoryGateFailurePolicy {
    Queued {
        orchestrator: Arc<dyn QueuedJobOrchestrator>,
    },
    FailJob,
}
