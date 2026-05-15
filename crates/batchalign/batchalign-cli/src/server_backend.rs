//! Server control-plane backend seam.
//!
//! This boundary keeps route handlers and other app-facing code from depending
//! directly on a specific orchestration backend. The [`ServerBackend`] trait
//! is the single interface that route handlers and lifecycle code consume.
//!
//! Two implementations exist, both production-supported:
//! - [`TemporalServerBackend`](crate::temporal_backend::TemporalServerBackend) —
//!   external-orchestrator backend backed by Temporal workflows and activities.
//!   Selected when `server.yaml` `temporal_server_url` is a non-empty URL.
//! - [`TestServerBackend`] — in-process backend that spawns inline runner tasks
//!   without external dependencies. Selected when `temporal_server_url` is
//!   empty / `"none"` / `"local"` / `"disabled"` (or omitted). Despite the
//!   `Test`-prefixed name (which dates from when this path was integration-test-only),
//!   this is the production non-Temporal backend; the fleet currently runs
//!   here on hosts that don't enable the `temporal_enabled` / `temporal_worker`
//!   host flags.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::api::{
    CancellationRequest, JobControlPlaneBackendKind, JobControlPlaneInfo, JobId, JobInfo,
    JobListItem, JobStatus, NodeId, NumWorkers, UnixTimestamp,
};
use crate::db::JobDB;
use crate::error::ServerError;
use crate::host_memory::HostMemoryError;
use crate::runner::util::RunnerEventSink;
use crate::runner::{
    ExecutionEngine, MemoryGateRejectionDisposition, QueuedJobOrchestrator, ServerExecutionHost,
    job_task,
};
use crate::runtime_supervisor::{RuntimeSupervisor, ShutdownError, ShutdownSummary};
use crate::scheduling::{DurationMs, RetryPolicy};
use crate::store::{Job, JobDetail, JobStore, unix_now};
use crate::types::traces::JobTraces;
use crate::ws::{BROADCAST_CAPACITY, WsEvent};

/// Store-backed health and queue-state snapshot for the server control plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerControlPlaneSnapshot {
    /// Node identifier used for queue-lease ownership.
    pub node_id: NodeId,
    /// Number of workers currently available to accept work.
    pub workers_available: i64,
    /// Number of jobs currently considered active by the control plane.
    pub active_jobs: i64,
    /// Number of unexpected worker crashes seen by this server.
    pub worker_crashes: i64,
    /// Number of work-unit attempts started.
    pub attempts_started: i64,
    /// Number of work-unit attempts retried.
    pub attempts_retried: i64,
    /// Number of deferred work units waiting for later eligibility.
    pub deferred_work_units: i64,
    /// Number of files forced into a terminal error state by the runner.
    pub forced_terminal_errors: i64,
    /// Number of jobs aborted by the host-memory gate.
    pub memory_gate_aborts: i64,
}

/// App-facing backend for queued-job orchestration and persisted job state.
#[async_trait]
pub trait ServerBackend: Send + Sync {
    /// Persist a newly submitted job and wake the dispatcher if needed.
    async fn submit_job(&self, job: Job) -> Result<(), ServerError>;

    /// Return the current list view for all known jobs.
    async fn list_jobs(&self) -> Vec<JobListItem>;

    /// Return one current job snapshot if it exists.
    async fn get_job(&self, job_id: &JobId) -> Option<JobInfo>;

    /// Return the detail projection used by results/traces routes.
    async fn get_job_detail(&self, job_id: &JobId) -> Option<JobDetail>;

    /// Return the current lifecycle status of one job.
    async fn job_status(&self, job_id: &JobId) -> Option<JobStatus>;

    /// Return whether the given job is still running.
    async fn is_job_running(&self, job_id: &JobId) -> Option<bool>;

    /// Request cancellation of one queued or running job.
    ///
    /// `provenance` carries caller-reported source/host/pid/reason that the
    /// store persists into the `cancellations` audit table. Callers wanting
    /// the legacy "anonymous" behavior pass `CancellationRequest::default()`,
    /// which records `source=Api` plus whatever the route handler enriched.
    async fn cancel_job(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError>;

    /// Record a cancel attempt against a job that is already in a terminal
    /// state. Persists the audit row with `accepted=false` so a forensic
    /// reader can count "user pressed cancel against a finished job."
    /// Does NOT call into the cancellation pathway (the job is already done).
    async fn record_terminal_cancel(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError>;

    /// Return every cancel attempt recorded for one job (oldest first).
    async fn list_cancellations(
        &self,
        job_id: &JobId,
    ) -> Result<Vec<crate::db::CancellationRow>, ServerError>;

    /// Permanently delete one terminal job.
    async fn delete_job(&self, job_id: &JobId) -> Result<(), ServerError>;

    /// Re-queue a restartable job and wake the dispatcher.
    async fn restart_job(&self, job_id: &JobId) -> Result<JobInfo, ServerError>;

    /// Interrupt all active jobs as part of graceful server shutdown.
    ///
    /// Returns the number of jobs that transitioned to `JobStatus::Interrupted`.
    /// Jobs land in `Interrupted` (resumable) rather than `Cancelled` (terminal)
    /// so the startup recovery path requeues unfinished work after a restart.
    async fn interrupt_all_for_shutdown(&self) -> usize;

    /// Return the store-backed control-plane health snapshot.
    async fn control_plane_snapshot(&self) -> ServerControlPlaneSnapshot;

    /// Return algorithm traces for one job if they were collected.
    async fn get_job_traces(&self, job_id: &JobId) -> Option<Arc<JobTraces>>;

    /// Subscribe to control-plane broadcast events.
    fn subscribe_events(&self) -> broadcast::Receiver<WsEvent>;

    /// Stop background control-plane tasks tracked by the runtime supervisor.
    async fn shutdown_runtime(&self, timeout: Duration) -> Result<ShutdownSummary, ShutdownError>;
}

/// Summary returned when bootstrapping one concrete server backend.
///
/// This keeps the server factory from needing direct access to backend-specific
/// queue/runtime/broadcast internals just to report basic startup information.
pub(crate) struct ServerBackendBootstrap {
    /// Route-facing backend over the fully started control plane.
    pub backend: Arc<dyn ServerBackend>,
    /// Number of persisted jobs loaded from the database at startup.
    pub loaded_jobs: usize,
    /// Number of queued jobs eligible for dispatch resume.
    pub queued_jobs: usize,
}

// ---------------------------------------------------------------------------
// TestServerBackend — in-process control-plane backend
//
// Despite the `Test`-prefixed name (which dates from when this path was
// integration-test-only), this is the production non-Temporal backend:
// `create_app_with_prepared_workers` selects it whenever
// `temporal_server_url` is empty / `"none"` / `"local"` / `"disabled"`.
// `create_test_app{,_with_prepared_workers}` also routes through it for
// integration tests that don't want a Temporal dependency.
// ---------------------------------------------------------------------------

/// Queued-job orchestrator paired with [`TestServerBackend`].
///
/// Handles memory-gate rejections by re-queueing with a backoff policy.
/// Both this and the Temporal orchestrator implement the same
/// [`QueuedJobOrchestrator`] trait so behavior stays consistent across
/// the two backends.
struct TestJobOrchestrator {
    memory_gate_retry_policy: RetryPolicy,
}

impl TestJobOrchestrator {
    fn new() -> Self {
        Self {
            memory_gate_retry_policy: RetryPolicy {
                max_attempts: 1,
                initial_backoff_ms: DurationMs(30_000),
                max_backoff_ms: DurationMs(120_000),
                backoff_multiplier: 2,
            },
        }
    }
}

#[async_trait]
impl QueuedJobOrchestrator for TestJobOrchestrator {
    async fn handle_memory_gate_rejection(
        &self,
        sink: &Arc<dyn RunnerEventSink>,
        job_id: &JobId,
        _requested_workers: NumWorkers,
        _error: &HostMemoryError,
    ) -> Result<MemoryGateRejectionDisposition, ServerError> {
        let retry_at = UnixTimestamp(
            unix_now().0 + (self.memory_gate_retry_policy.backoff_for_retry(1).0 as f64 / 1000.0),
        );
        sink.requeue_job_after_memory_gate(job_id, retry_at).await;
        sink.bump_deferred_work_units().await;
        Ok(MemoryGateRejectionDisposition::Requeued { retry_at })
    }
}

/// In-process server control-plane backend.
///
/// Selected by [`create_app_with_prepared_workers`](crate::server::create_app_with_prepared_workers)
/// when `temporal_server_url` resolves to [`TemporalBackend::Disabled`](crate::config::TemporalBackend::Disabled),
/// and unconditionally by [`create_test_app_with_prepared_workers`](crate::server::create_test_app_with_prepared_workers).
/// Spawns inline runner tasks via `tokio::spawn` and requires no external
/// service. Each submitted job is persisted to the store and immediately
/// dispatched on the local Tokio runtime.
///
/// Trade-off vs the Temporal backend: in-flight jobs do **not** survive a
/// `batchalign3` server restart here, because the control plane lives
/// inside this process. For deploy-tolerant in-flight execution use the
/// Temporal backend ([`TemporalServerBackend`](crate::temporal_backend::TemporalServerBackend)).
pub(crate) struct TestServerBackend {
    store: Arc<JobStore>,
    host: ServerExecutionHost,
    runtime: RuntimeSupervisor,
    ws_tx: broadcast::Sender<WsEvent>,
    config: crate::config::ServerConfig,
    jobs_dir: std::path::PathBuf,
    /// Cached hostname for staged remote job provenance. Computed once at startup.
    hostname: String,
}

fn test_control_plane_info() -> JobControlPlaneInfo {
    JobControlPlaneInfo {
        backend: JobControlPlaneBackendKind::Test,
        temporal: None,
    }
}

#[async_trait]
impl ServerBackend for TestServerBackend {
    async fn submit_job(&self, job: Job) -> Result<(), ServerError> {
        let job_id = job.identity.job_id.clone();

        // Auto-route: if this machine belongs to a fleet and the job has
        // local media that isn't visible on the fleet server, stage it.
        // If no fleet_target is configured (external users), always local.
        if let Some(fleet) = &self.config.fleet_target
            && job.filesystem.paths_mode
        {
            // Check if media is local-only (not NFS-visible on the fleet server).
            // For now, any paths-mode job on a machine with fleet_target configured
            // gets staged — the fleet server is always faster.
            // TODO: probe whether source_paths are NFS-visible and skip staging
            // for those (an operator's workflow where data is already on NFS).
            let source_paths: Vec<std::path::PathBuf> = job
                .filesystem
                .source_paths
                .iter()
                .map(|p| std::path::PathBuf::from(p.as_str()))
                .collect();
            let output_dir = job
                .filesystem
                .output_paths
                .first()
                .map(|p| {
                    std::path::PathBuf::from(p.as_str())
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf()
                })
                .unwrap_or_else(|| self.jobs_dir.clone());

            let cancel_token = job.runtime.cancel_token.clone();
            let submission_template = crate::types::request::JobSubmission {
                command: job.dispatch.command,
                lang: job.dispatch.lang.clone(),
                num_speakers: job.dispatch.num_speakers,
                options: job.dispatch.options.clone(),
                paths_mode: true,
                source_paths: Vec::new(),
                output_paths: Vec::new(),
                source_dir: Default::default(),
                debug_traces: false,
                files: Vec::new(),
                media_files: Vec::new(),
                display_names: Vec::new(),
                before_paths: Vec::new(),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
            };

            self.store.submit(job).await?;

            let staging_config = fleet.clone();

            let store = self.store.clone();
            let jobs_dir = self.jobs_dir.clone();
            let hostname = self.hostname.clone();
            self.runtime
                .spawn_detached(crate::staging::run_staged_remote_job(
                    store,
                    job_id,
                    staging_config,
                    source_paths,
                    output_dir,
                    jobs_dir,
                    hostname,
                    cancel_token,
                    submission_template,
                ));
            return Ok(());
        }

        // Default: local execution (no fleet target, or non-paths-mode job)
        self.store.submit(job).await?;
        let host = self.host.clone();
        self.runtime.spawn_detached(job_task(job_id, host));
        Ok(())
    }

    async fn list_jobs(&self) -> Vec<JobListItem> {
        self.store
            .list_all()
            .await
            .into_iter()
            .map(|job| job.with_control_plane(test_control_plane_info()))
            .collect()
    }

    async fn get_job(&self, job_id: &JobId) -> Option<JobInfo> {
        self.store
            .get(job_id)
            .await
            .map(|job| job.with_control_plane(test_control_plane_info()))
    }

    async fn get_job_detail(&self, job_id: &JobId) -> Option<JobDetail> {
        self.store.get_job_detail(job_id).await
    }

    async fn job_status(&self, job_id: &JobId) -> Option<JobStatus> {
        self.store.job_status(job_id).await
    }

    async fn is_job_running(&self, job_id: &JobId) -> Option<bool> {
        self.store.is_running(job_id).await
    }

    async fn cancel_job(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError> {
        self.store.cancel(job_id, provenance).await?;
        // Reap in-flight workers so dispatch futures unwind instead
        // of awaiting natural completion of the current ML call.
        self.host.pool().shutdown_workers_for_job(job_id).await;
        Ok(())
    }

    async fn record_terminal_cancel(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError> {
        self.store.record_terminal_cancel(job_id, provenance).await
    }

    async fn list_cancellations(
        &self,
        job_id: &JobId,
    ) -> Result<Vec<crate::db::CancellationRow>, ServerError> {
        self.store.list_cancellations(job_id).await
    }

    async fn delete_job(&self, job_id: &JobId) -> Result<(), ServerError> {
        self.store.delete(job_id).await
    }

    async fn restart_job(&self, job_id: &JobId) -> Result<JobInfo, ServerError> {
        let info = self.store.restart(job_id).await?;
        let host = self.host.clone();
        let restart_job_id = job_id.clone();
        self.runtime.spawn_detached(job_task(restart_job_id, host));
        Ok(info.with_control_plane(test_control_plane_info()))
    }

    async fn interrupt_all_for_shutdown(&self) -> usize {
        self.store.interrupt_all_for_shutdown().await
    }

    async fn control_plane_snapshot(&self) -> ServerControlPlaneSnapshot {
        store_backed_control_plane_snapshot(self.store.as_ref()).await
    }

    async fn get_job_traces(&self, job_id: &JobId) -> Option<Arc<JobTraces>> {
        self.store.trace_store().get(job_id).await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<WsEvent> {
        self.ws_tx.subscribe()
    }

    async fn shutdown_runtime(&self, timeout: Duration) -> Result<ShutdownSummary, ShutdownError> {
        self.runtime.shutdown(timeout).await
    }
}

/// Build and start the in-process server control plane.
///
/// Used by both production (via
/// [`create_app_with_prepared_workers`](crate::server::create_app_with_prepared_workers)
/// when `temporal_server_url` resolves to
/// [`TemporalBackend::Disabled`](crate::config::TemporalBackend::Disabled))
/// and by integration tests (via
/// [`create_test_app_with_prepared_workers`](crate::server::create_test_app_with_prepared_workers)).
/// Unlike [`bootstrap_temporal_server_backend`](crate::temporal_backend::bootstrap_temporal_server_backend),
/// this requires no external services: queued-job orchestration runs as
/// inline `tokio::spawn` runner tasks against the local [`JobStore`].
///
/// Performs the same startup recovery contract as the Temporal path:
/// loads jobs from the SQLite store, identifies any `Queued` rows whose
/// runner died with the previous server process, and spawns a fresh
/// `job_task` for each so they don't stall indefinitely.
pub(crate) async fn bootstrap_test_server_backend(
    config: crate::config::ServerConfig,
    db: Arc<JobDB>,
    engine: ExecutionEngine,
    jobs_dir: std::path::PathBuf,
) -> Result<ServerBackendBootstrap, ServerError> {
    let (ws_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(config.clone(), Some(db), ws_tx.clone()));
    let loaded_jobs = store.load_from_db().await?;

    // Collect queued job IDs before consuming store/host/runtime into the backend struct.
    // These are jobs that were Queued at shutdown or requeued by crash recovery; they have
    // no active job_task and would stay Queued indefinitely without explicit dispatch here.
    let queued_job_ids = store.queued_job_ids().await;
    let queued_jobs = queued_job_ids.len();

    let orchestrator = Arc::new(TestJobOrchestrator::new());
    let runtime = RuntimeSupervisor::new();
    let host = ServerExecutionHost::new(store.clone(), engine, orchestrator);

    // Clone before the move into TestServerBackend so we can dispatch queued jobs below.
    let runtime_for_resume = runtime.clone();
    let host_for_resume = host.clone();

    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string());
    let backend: Arc<dyn ServerBackend> = Arc::new(TestServerBackend {
        store,
        host,
        runtime,
        ws_tx,
        config,
        jobs_dir,
        hostname,
    });

    // Fulfill the recovery promise: spawn job_task for every queued job that has no
    // active runner. This covers two cases:
    //   1. Startup recovery: crash-interrupted jobs re-queued by load_from_db() with
    //      partially-completed work that can be resumed.
    //   2. Memory-gate requeue: jobs that were deferred by the memory gate in a previous
    //      daemon session and never re-dispatched because job_task exited without a
    //      scheduler picking them up (see also: job_task Requeued handling below).
    for job_id in queued_job_ids {
        runtime_for_resume.spawn_detached(job_task(job_id, host_for_resume.clone()));
    }

    Ok(ServerBackendBootstrap {
        backend,
        loaded_jobs,
        queued_jobs,
    })
}

/// Store-backed control-plane snapshot shared by all non-embedded backends.
pub(crate) async fn store_backed_control_plane_snapshot(
    store: &JobStore,
) -> ServerControlPlaneSnapshot {
    let (
        worker_crashes,
        attempts_started,
        attempts_retried,
        deferred_work_units,
        forced_terminal_errors,
        memory_gate_aborts,
    ) = store.operational_counters().await;
    let workers_available = store.workers_available().await;
    let active_jobs = store.active_jobs().await;
    ServerControlPlaneSnapshot {
        node_id: store.node_id().clone(),
        workers_available,
        active_jobs,
        worker_crashes,
        attempts_started,
        attempts_retried,
        deferred_work_units,
        forced_terminal_errors,
        memory_gate_aborts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::time::Duration;

    use tokio_util::sync::CancellationToken;

    use crate::api::{
        CorrelationId, DisplayPath, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
    };
    use crate::cache::UtteranceCache;
    use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};
    use crate::runner::{ExecutionEngine, RunnerExecutionContext};
    use crate::store::{
        FileStatus, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity,
        JobLeaseState, JobRuntimeControl, JobScheduleState, JobSourceContext, unix_now,
    };
    use crate::worker::pool::{PoolConfig, WorkerPool};

    fn sample_job(job_id: &str) -> Job {
        let filename = "sample.cha";
        let mut file_statuses = HashMap::new();
        file_statuses.insert(
            filename.to_string(),
            FileStatus::new(DisplayPath::from(filename)),
        );

        Job {
            identity: JobIdentity {
                job_id: JobId::from(job_id),
                correlation_id: CorrelationId::from(format!("test-{job_id}")),
            },
            dispatch: JobDispatchConfig {
                command: ReleasedCommand::Morphotag,
                lang: LanguageSpec::Resolved(LanguageCode3::eng()),
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Morphotag(MorphotagOptions {
                    common: CommonOptions::default(),

                    ..Default::default()
                }),
                runtime_state: Default::default(),
                debug_traces: false,
            },
            source: JobSourceContext {
                submitted_by: "127.0.0.1".into(),
                submitted_by_name: String::new(),
                source_dir: Default::default(),
            },
            filesystem: JobFilesystemConfig {
                filenames: vec![DisplayPath::from(filename)],
                has_chat: vec![true],
                staging_dir: Default::default(),
                paths_mode: false,
                source_paths: Vec::new(),
                output_paths: Vec::new(),
                before_paths: Vec::new(),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: Default::default(),
            },
            execution: JobExecutionState {
                status: JobStatus::Queued,
                file_statuses,
                results: Vec::new(),
                error: None,
                completed_files: 0,
                batch_progress: None,
            },
            schedule: JobScheduleState {
                submitted_at: unix_now(),
                completed_at: None,
                next_eligible_at: None,
                num_workers: None,
                lease: JobLeaseState {
                    leased_by_node: None,
                    expires_at: None,
                    heartbeat_at: None,
                },
                last_cancel: None,
            },
            runtime: JobRuntimeControl {
                cancel_token: CancellationToken::new(),
                runner_active: false,
            },
            execution_plan: None,
        }
    }

    #[tokio::test]
    async fn test_backend_submit_dispatches_job() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = crate::config::RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        std::fs::create_dir_all(layout.state_dir()).expect("create state dir");

        let db = Arc::new(
            crate::db::JobDB::open_with_layout(&layout, Some(layout.state_dir()))
                .await
                .expect("open job db"),
        );
        let pool = Arc::new(WorkerPool::new(PoolConfig {
            test_echo: true,
            ..Default::default()
        }));
        pool.start_background_tasks();
        let cache = Arc::new(
            UtteranceCache::sqlite(Some(tempdir.path().join("cache")))
                .await
                .expect("open cache"),
        );
        let engine = ExecutionEngine::new(RunnerExecutionContext::new(
            pool,
            cache,
            Vec::new(),
            BTreeMap::new(),
            true,
        ));

        let bootstrap = bootstrap_test_server_backend(
            crate::config::ServerConfig::default(),
            db,
            engine,
            tempdir.path().join("jobs"),
        )
        .await
        .expect("bootstrap test backend");

        bootstrap
            .backend
            .submit_job(sample_job("job-test-dispatch"))
            .await
            .expect("test backend should submit the job");

        // Verify the job exists in the store.
        assert!(
            bootstrap
                .backend
                .get_job(&JobId::from("job-test-dispatch"))
                .await
                .is_some(),
            "test backend should expose submitted job state"
        );

        bootstrap
            .backend
            .shutdown_runtime(Duration::from_secs(5))
            .await
            .expect("shutdown runtime");
    }
}
