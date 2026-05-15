//! Temporal-backed server control plane.
//!
//! This is an alternate server backend that keeps the existing Batchalign
//! execution engine and worker pool, but hands queued-job orchestration to
//! Temporal workflows and activities instead of the embedded local queue.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::anyhow;
use async_trait::async_trait;
use temporalio_client::errors::{WorkflowInteractionError, WorkflowStartError};
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, WorkflowCancelOptions,
    WorkflowDescribeOptions, WorkflowStartOptions, WorkflowTerminateOptions,
};
use temporalio_common::protos::temporal::api::enums::v1::{
    WorkflowExecutionStatus, WorkflowIdConflictPolicy, WorkflowIdReusePolicy,
};
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::activities::{ActivityContext, ActivityError};
use temporalio_sdk::{
    ActivityOptions, ApplicationFailure, Worker, WorkerOptions, WorkflowContext,
    WorkflowContextView, WorkflowResult,
};
use temporalio_sdk_core::{CoreRuntime, RuntimeOptions, Url};
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::api::{
    CancelReason, CancelSource, CancellationRequest, JobControlPlaneInfo, JobId, JobInfo,
    JobListItem, JobStatus, NumWorkers, TemporalWorkflowExecutionInfo, UnixTimestamp,
};
use crate::config::ServerConfig;
use crate::db::JobDB;
use crate::error::ServerError;
use crate::host_memory::HostMemoryError;
use crate::runner::util::RunnerEventSink;
use crate::runner::{
    ExecutionEngine, MemoryGateRejectionDisposition, QueuedJobOrchestrator, ServerExecutionHost,
    job_task,
};
use crate::runtime_supervisor::{
    RuntimeSupervisor, ShutdownError, ShutdownSummary, SpawnedTaskOutcome,
};
use crate::scheduling::{DurationMs, RetryPolicy};
use crate::server_backend::{
    ServerBackend, ServerBackendBootstrap, ServerControlPlaneSnapshot,
    store_backed_control_plane_snapshot,
};
use crate::store::{Job, JobDetail, JobStore, unix_now};
use crate::temporal_reconciler::{TemporalStateQuery, TemporalWorkflowOutcome};

/// Seconds between background reconcile ticks. Bounded staleness on
/// dashboards and conflict-detection state for the whole daemon.
const RECONCILER_TICK_S: u64 = 30;
/// Seconds a job must wait beyond its submission before the reconciler
/// is allowed to sweep it to `Failed` on the grounds that Temporal says
/// the workflow doesn't exist. Comfortably larger than workflow
/// start-visibility latency so transient describe misses don't kill
/// freshly submitted work.
const RECONCILER_STALE_THRESHOLD_S: u64 = 300;
use crate::types::traces::JobTraces;
use crate::ws::{BROADCAST_CAPACITY, WsEvent};

/// Concrete `TemporalStateQuery` that wraps a Temporal client. Translates
/// `WorkflowExecutionStatus` into the four-variant `TemporalWorkflowOutcome`
/// the reconciler's pure decision function consumes. `NotFound` at the
/// transport layer maps to `Ok(None)`; other transport errors propagate.
struct TemporalClientStateQuery {
    client: Client,
}

impl TemporalClientStateQuery {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl TemporalStateQuery for TemporalClientStateQuery {
    async fn query_workflow_outcome(
        &self,
        job_id: &JobId,
    ) -> Result<Option<TemporalWorkflowOutcome>, ServerError> {
        match self
            .client
            .get_workflow_handle::<BatchalignJobWorkflow>(job_id.to_string())
            .describe(WorkflowDescribeOptions::default())
            .await
        {
            Ok(description) => {
                let info = description.raw_description.workflow_execution_info;
                Ok(info
                    .as_ref()
                    .map(|i| i.status())
                    .and_then(map_temporal_status_to_outcome))
            }
            Err(WorkflowInteractionError::NotFound(_)) => Ok(None),
            Err(err) => Err(ServerError::Validation(format!(
                "Temporal describe for {job_id} failed: {err}"
            ))),
        }
    }
}

/// Map the full `WorkflowExecutionStatus` enum to the 4-variant outcome
/// the reconciler's decision function consumes. `ContinuedAsNew` and
/// `Unspecified` have no clean mapping — they return `None`, which the
/// reconciler treats as "no actionable verdict yet, try again next tick."
fn map_temporal_status_to_outcome(
    status: WorkflowExecutionStatus,
) -> Option<TemporalWorkflowOutcome> {
    match status {
        WorkflowExecutionStatus::Running | WorkflowExecutionStatus::Paused => {
            Some(TemporalWorkflowOutcome::Active)
        }
        WorkflowExecutionStatus::Completed => Some(TemporalWorkflowOutcome::Completed),
        WorkflowExecutionStatus::Canceled | WorkflowExecutionStatus::Terminated => {
            Some(TemporalWorkflowOutcome::Cancelled)
        }
        WorkflowExecutionStatus::Failed | WorkflowExecutionStatus::TimedOut => {
            Some(TemporalWorkflowOutcome::Failed)
        }
        WorkflowExecutionStatus::ContinuedAsNew | WorkflowExecutionStatus::Unspecified => None,
    }
}

/// Temporal workflow input for one Batchalign job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemporalJobWorkflowInput {
    job_id: String,
    activity_timeout_s: u64,
    heartbeat_timeout_s: u64,
}

/// Temporal activity input for one Batchalign job attempt.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemporalJobActivityInput {
    job_id: String,
}

/// Serializable outcome returned from the Temporal activity back into the workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum TemporalJobActivityOutcome {
    /// The shared Batchalign engine finished the job attempt successfully.
    Completed,
    /// The shared engine asked the workflow to sleep before trying again.
    Requeued { retry_after_ms: u64 },
}

#[workflow]
struct BatchalignJobWorkflow {
    job_id: String,
    activity_timeout_s: u64,
    heartbeat_timeout_s: u64,
}

#[workflow_methods]
impl BatchalignJobWorkflow {
    #[init]
    fn new(_ctx: &WorkflowContextView, input: TemporalJobWorkflowInput) -> Self {
        Self {
            job_id: input.job_id,
            activity_timeout_s: input.activity_timeout_s,
            heartbeat_timeout_s: input.heartbeat_timeout_s,
        }
    }

    #[run]
    async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<()> {
        loop {
            let input = TemporalJobActivityInput {
                job_id: ctx.state(|state| state.job_id.clone()),
            };
            let activity_timeout_s = ctx.state(|state| state.activity_timeout_s.max(1));
            let heartbeat_timeout_s = ctx.state(|state| state.heartbeat_timeout_s.max(1));
            let activity_options = ActivityOptions::with_start_to_close_timeout(
                Duration::from_secs(activity_timeout_s),
            )
            .heartbeat_timeout(Duration::from_secs(heartbeat_timeout_s))
            .build();
            // Surface workflow status in the Temporal UI so operators can
            // distinguish an in-flight activity from a between-retries wait
            // without inspecting logs. The workflow only knows these two
            // coarse states; per-file progress lives in the activity's
            // heartbeat payload.
            ctx.set_current_details("running job attempt");
            let outcome = ctx
                .start_activity(
                    BatchalignTemporalActivities::run_job_attempt,
                    input,
                    activity_options,
                )
                .await?;
            match outcome {
                TemporalJobActivityOutcome::Completed => return Ok(()),
                TemporalJobActivityOutcome::Requeued { retry_after_ms } => {
                    ctx.set_current_details(format!("queued for retry in {retry_after_ms}ms"));
                    ctx.timer(Duration::from_millis(retry_after_ms.max(1)))
                        .await;
                }
            }
        }
    }
}

/// Activity bridge from Temporal into the shared Batchalign execution engine.
///
/// The Temporal worker runs on a dedicated OS thread with its own
/// `current_thread` + `LocalSet` tokio runtime. Oneshot reply channels
/// across runtime boundaries are unreliable, so the activity dispatches
/// `job_task()` to the **main runtime** via `RuntimeSupervisor::spawn_job()`
/// and awaits a `oneshot` completion signal. Cancel signals from Temporal
/// are forwarded the same way, ensuring the local `JobStore` is only ever
/// accessed from the runtime where its actor lives.
///
/// Relies on the per-host task-queue invariant: each server polls
/// `batchalign3-{hostname}`, so any workflow's activities land on the
/// server whose local `JobStore` persisted the job. See the
/// `architecture/temporal-fleet-topology.md` book page for the full
/// topology, failure mode, and future cross-fleet-distribution design.
#[derive(Clone)]
pub struct BatchalignTemporalActivities {
    host: ServerExecutionHost,
    /// Supervisor running on the main server runtime. `spawn_job()` sends
    /// a task to its `JoinSet` and returns a `oneshot::Receiver` that fires
    /// when the task completes.
    runtime: RuntimeSupervisor,
}

#[activities]
impl BatchalignTemporalActivities {
    /// Execute one job attempt as a Temporal activity.
    ///
    /// Dispatches `job_task()` to the main server runtime via
    /// `RuntimeSupervisor::spawn_job()`, then awaits the completion signal
    /// while heartbeating to Temporal. Cancel signals are forwarded to the
    /// main runtime via a separate `spawn_job()` call.
    ///
    /// No `JobStore` methods are called from the worker thread — all store
    /// access happens on the main runtime where the actor is reachable.
    #[activity]
    pub async fn run_job_attempt(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: TemporalJobActivityInput,
    ) -> Result<TemporalJobActivityOutcome, ActivityError> {
        let job_id = JobId::from(input.job_id);
        info!(job_id = %job_id, "Temporal activity: dispatching job_task to main runtime");

        let host = self.host.clone();
        let task_job_id = job_id.clone();
        let mut done = self.runtime.spawn_job(job_task(task_job_id, host));

        let heartbeat_every = ctx
            .info()
            .heartbeat_timeout
            .map(|timeout| std::cmp::max(Duration::from_secs(1), timeout / 2))
            .unwrap_or_else(|| Duration::from_secs(5));
        let mut heartbeat = tokio::time::interval(heartbeat_every);
        let mut forwarded_cancel = false;

        loop {
            tokio::select! {
                outcome = &mut done => {
                    match outcome {
                        SpawnedTaskOutcome::Completed => {
                            info!(job_id = %job_id, "Temporal activity: job_task completed on main runtime");
                            return Ok(TemporalJobActivityOutcome::Completed);
                        }
                        SpawnedTaskOutcome::NotSpawned => {
                            warn!(job_id = %job_id, "Temporal activity: supervisor rejected task");
                            return Err(ActivityError::Application(Box::new(
                                ApplicationFailure::non_retryable(anyhow!(
                                    "RuntimeSupervisor rejected job {job_id}"
                                )),
                            )));
                        }
                        SpawnedTaskOutcome::ChannelDropped => {
                            warn!(job_id = %job_id, "Temporal activity: completion channel dropped");
                            return Err(ActivityError::Application(Box::new(
                                ApplicationFailure::non_retryable(anyhow!(
                                    "job {job_id} completion channel dropped"
                                )),
                            )));
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    ctx.record_heartbeat(Vec::new());
                    if ctx.is_cancelled() && !forwarded_cancel {
                        forwarded_cancel = true;
                        info!(job_id = %job_id, "Temporal activity: forwarding cancel to main runtime");
                        let store = self.host.store().clone();
                        let pool = self.host.pool().clone();
                        let cancel_job_id = job_id.clone();
                        // The originating cancel was already audited at the
                        // route handler. This forward is internal — record
                        // it with a Signal source + a marker reason so the
                        // audit-table reader can distinguish "user cancel"
                        // from "Temporal-detected forward."
                        let inner_provenance = CancellationRequest {
                            source: Some(CancelSource::Signal),
                            reason: Some(CancelReason::temporal_activity_forwarded()),
                            ..Default::default()
                        };
                        self.runtime.spawn_detached(async move {
                            let _ = store.cancel(&cancel_job_id, inner_provenance).await;
                            pool.shutdown_workers_for_job(&cancel_job_id).await;
                        });
                    }
                }
            }
        }
    }
}

/// Temporal-specific queued-job orchestrator used by the shared runner.
struct TemporalJobOrchestrator {
    memory_gate_retry_policy: RetryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemporalWorkflowStartMode {
    ResumeOrUseExisting,
    ReplaceExisting,
}

impl TemporalJobOrchestrator {
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
impl QueuedJobOrchestrator for TemporalJobOrchestrator {
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

/// Real Temporal-backed server backend over one JobStore projection.
pub struct TemporalServerBackend {
    store: Arc<JobStore>,
    client: Client,
    worker_runtime: TemporalWorkerRuntime,
    ws_tx: broadcast::Sender<WsEvent>,
    config: ServerConfig,
    reconciler: Arc<crate::temporal_reconciler::TemporalReconciler>,
}

impl TemporalServerBackend {
    fn new(
        store: Arc<JobStore>,
        client: Client,
        worker_runtime: TemporalWorkerRuntime,
        ws_tx: broadcast::Sender<WsEvent>,
        config: ServerConfig,
    ) -> Self {
        // The reconciler propagates Temporal's authoritative workflow
        // status back into the local store. See
        // `crate::temporal_reconciler` for the contract.
        let query = Arc::new(TemporalClientStateQuery::new(client.clone()));
        // Stale threshold: workflow start-visibility latency in Temporal is
        // sub-second in practice; 5 min gives enough grace for transient
        // describe failures and daemon-restart races before sweeping a
        // "not found" job to `Failed`.
        let reconciler = Arc::new(crate::temporal_reconciler::TemporalReconciler::new(
            store.clone(),
            query,
            RECONCILER_STALE_THRESHOLD_S,
        ));
        Self {
            store,
            client,
            worker_runtime,
            ws_tx,
            config,
            reconciler,
        }
    }

    /// Access to the reconciler for bootstrap (spawn periodic tick).
    pub(crate) fn reconciler(&self) -> Arc<crate::temporal_reconciler::TemporalReconciler> {
        self.reconciler.clone()
    }

    async fn ensure_workflow_for_job(
        &self,
        job_id: &JobId,
        start_mode: TemporalWorkflowStartMode,
    ) -> Result<(), ServerError> {
        let input = TemporalJobWorkflowInput {
            job_id: job_id.to_string(),
            activity_timeout_s: self.config.temporal_activity_timeout_s,
            heartbeat_timeout_s: self.config.temporal_heartbeat_s,
        };
        let options = match start_mode {
            TemporalWorkflowStartMode::ResumeOrUseExisting => WorkflowStartOptions::new(
                self.config.temporal_task_queue.clone(),
                job_id.to_string(),
            )
            .id_reuse_policy(WorkflowIdReusePolicy::AllowDuplicate)
            .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
            .build(),
            TemporalWorkflowStartMode::ReplaceExisting => WorkflowStartOptions::new(
                self.config.temporal_task_queue.clone(),
                job_id.to_string(),
            )
            .id_reuse_policy(WorkflowIdReusePolicy::AllowDuplicate)
            .id_conflict_policy(WorkflowIdConflictPolicy::TerminateExisting)
            .build(),
        };
        match self
            .client
            .start_workflow(BatchalignJobWorkflow::run, input, options)
            .await
        {
            Ok(_) => Ok(()),
            Err(WorkflowStartError::AlreadyStarted { .. }) => Ok(()),
            Err(error) => Err(ServerError::Persistence(format!(
                "failed to start Temporal workflow for job {job_id}: {error}"
            ))),
        }
    }

    async fn bootstrap_active_workflows(&self) -> Result<(), ServerError> {
        for job in self
            .store
            .list_all()
            .await
            .into_iter()
            .filter(|job| job.status.is_active())
        {
            self.ensure_workflow_for_job(
                &job.job_id,
                TemporalWorkflowStartMode::ResumeOrUseExisting,
            )
            .await?;
        }
        Ok(())
    }

    async fn cancel_temporal_workflow(&self, job_id: &JobId) -> Result<(), ServerError> {
        let handle = self
            .client
            .get_workflow_handle::<BatchalignJobWorkflow>(job_id.to_string());
        match handle
            .cancel(
                WorkflowCancelOptions::builder()
                    .reason(format!("batchalign job {job_id} cancelled"))
                    .build(),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(WorkflowInteractionError::NotFound(_)) => Ok(()),
            Err(error) => Err(ServerError::Persistence(format!(
                "failed to cancel Temporal workflow for job {job_id}: {error}"
            ))),
        }
    }

    async fn terminate_temporal_workflow(
        &self,
        job_id: &JobId,
        reason: &str,
    ) -> Result<(), ServerError> {
        let handle = self
            .client
            .get_workflow_handle::<BatchalignJobWorkflow>(job_id.to_string());
        match handle
            .terminate(
                WorkflowTerminateOptions::builder()
                    .reason(reason.to_string())
                    .build(),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(WorkflowInteractionError::NotFound(_)) => Ok(()),
            Err(error) => Err(ServerError::Persistence(format!(
                "failed to terminate Temporal workflow for job {job_id}: {error}"
            ))),
        }
    }

    async fn describe_temporal_workflow(
        &self,
        job_id: &JobId,
    ) -> Result<temporalio_client::WorkflowExecutionDescription, WorkflowInteractionError> {
        self.client
            .get_workflow_handle::<BatchalignJobWorkflow>(job_id.to_string())
            .describe(WorkflowDescribeOptions::default())
            .await
    }

    async fn temporal_workflow_execution_info(
        &self,
        job_id: &JobId,
    ) -> TemporalWorkflowExecutionInfo {
        match self.describe_temporal_workflow(job_id).await {
            Ok(description) => {
                let info = description.raw_description.workflow_execution_info;
                let workflow_id = info
                    .as_ref()
                    .and_then(|info| info.execution.as_ref())
                    .and_then(|execution| non_empty_string(&execution.workflow_id))
                    .unwrap_or_else(|| job_id.to_string());
                TemporalWorkflowExecutionInfo {
                    workflow_id,
                    run_id: info
                        .as_ref()
                        .and_then(|info| info.execution.as_ref())
                        .and_then(|execution| non_empty_string(&execution.run_id)),
                    status: info
                        .as_ref()
                        .map(|info| temporal_workflow_status_name(info.status()).to_string()),
                    task_queue: info
                        .as_ref()
                        .and_then(|info| non_empty_string(&info.task_queue)),
                    history_length: info.as_ref().map(|info| info.history_length),
                    describe_error: None,
                }
            }
            Err(WorkflowInteractionError::NotFound(_)) => TemporalWorkflowExecutionInfo {
                workflow_id: job_id.to_string(),
                run_id: None,
                status: Some("not-found".into()),
                task_queue: None,
                history_length: None,
                describe_error: Some("Workflow not found".into()),
            },
            Err(error) => TemporalWorkflowExecutionInfo {
                workflow_id: job_id.to_string(),
                run_id: None,
                status: None,
                task_queue: None,
                history_length: None,
                describe_error: Some(error.to_string()),
            },
        }
    }

    async fn enrich_job_info(&self, job: JobInfo) -> JobInfo {
        let job_id = job.job_id.clone();
        job.with_control_plane(JobControlPlaneInfo::temporal_with_execution(
            self.temporal_workflow_execution_info(&job_id).await,
        ))
    }
}

/// Owned host thread for the in-process Temporal worker.
struct TemporalWorkerRuntime {
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    join_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl TemporalWorkerRuntime {
    fn start(
        config: &ServerConfig,
        client: Client,
        activities: BatchalignTemporalActivities,
    ) -> Result<Self, ServerError> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let temporal_config = config.clone();
        // Daemon-init invariants: this whole block runs once at
        // server bootstrap on a known-good `temporal_config` that
        // was validated by `EffectiveConfig::resolve` upstream. The
        // four expects below cover (a) tokio runtime construction,
        // which fails only on OS-level resource exhaustion that
        // would already have killed the daemon; (b) Temporal
        // `RuntimeOptions::build`, which validates the heartbeat
        // interval — guaranteed positive by the same upstream
        // validation; (c) `CoreRuntime::new_assume_tokio`, which
        // requires a tokio runtime present in scope (block_on
        // above); (d) `Worker::new`, which validates the
        // `WorkerOptions` shape, also fixed at compile time. None
        // are user-input-driven.
        #[allow(clippy::expect_used)]
        let join_handle = std::thread::Builder::new()
            .name("batchalign3-temporal-worker".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Temporal worker thread should build tokio runtime");
                let local = tokio::task::LocalSet::new();
                runtime.block_on(local.run_until(async move {
                    let core_runtime = CoreRuntime::new_assume_tokio(
                        RuntimeOptions::builder()
                            .heartbeat_interval(Some(Duration::from_secs(
                                temporal_config.temporal_heartbeat_s,
                            )))
                            .build()
                            .expect("Temporal worker runtime options should validate"),
                    )
                    .expect("Temporal worker core runtime should initialize");
                    let worker_options =
                        WorkerOptions::new(temporal_config.temporal_task_queue.clone())
                            .register_activities(activities)
                            .register_workflow::<BatchalignJobWorkflow>()
                            .build();
                    let mut worker = Worker::new(&core_runtime, client, worker_options)
                        .expect("Temporal worker should initialize");
                    let shutdown = worker.shutdown_handle();
                    tokio::spawn(async move {
                        let _ = shutdown_rx.await;
                        shutdown();
                    });
                    if let Err(error) = worker.run().await {
                        warn!(error = %error, "Temporal worker stopped with error");
                    }
                }));
            })
            .map_err(|error| {
                ServerError::Persistence(format!("failed to spawn Temporal worker thread: {error}"))
            })?;
        Ok(Self {
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
            join_handle: Mutex::new(Some(join_handle)),
        })
    }

    async fn shutdown(&self, timeout: Duration) -> Result<ShutdownSummary, ShutdownError> {
        // Mutex poisoning here would mean a panic crossed the
        // Mutex's guard — i.e. a panic somewhere else in the daemon
        // already corrupted state. Propagating that as a panic at
        // shutdown is the right behavior; recovering from a poisoned
        // shutdown lock would risk shipping bad results.
        #[allow(clippy::expect_used)]
        if let Some(sender) = self
            .shutdown_tx
            .lock()
            .expect("Temporal worker shutdown lock poisoned")
            .take()
        {
            let _ = sender.send(());
        }
        #[allow(clippy::expect_used)]
        let Some(join_handle) = self
            .join_handle
            .lock()
            .expect("Temporal worker join lock poisoned")
            .take()
        else {
            return Ok(ShutdownSummary {
                timed_out: false,
                remaining_jobs: 0,
            });
        };

        match tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                let _ = join_handle.join();
            }),
        )
        .await
        {
            Ok(join_result) => {
                let _ = join_result;
                Ok(ShutdownSummary {
                    timed_out: false,
                    remaining_jobs: 0,
                })
            }
            Err(_) => Ok(ShutdownSummary {
                timed_out: true,
                remaining_jobs: 1,
            }),
        }
    }
}

#[async_trait]
impl ServerBackend for TemporalServerBackend {
    async fn submit_job(&self, job: Job) -> Result<(), ServerError> {
        let job_id = job.identity.job_id.clone();

        // Opportunistic reconcile scoped to this submitter: close the
        // race window between reconciler ticks and new submissions.
        // Without it a stale `Queued` job whose workflow completed on
        // another worker would fire a spurious 409 Conflict here. Cost
        // is bounded — we only walk jobs for this `submitted_by`.
        let submitter = job.source.submitted_by.clone();
        let report = self.reconciler.reconcile_submitter(&submitter).await;
        if report.updated > 0 {
            tracing::debug!(
                submitter = %submitter,
                updated = report.updated,
                unchanged = report.unchanged,
                errored = report.errored,
                "Reconciled submitter's stale jobs before conflict detection"
            );
        }

        self.store.submit(job).await?;
        if let Err(error) = self
            .ensure_workflow_for_job(&job_id, TemporalWorkflowStartMode::ResumeOrUseExisting)
            .await
        {
            self.store
                .fail_job(&job_id, &error.to_string(), unix_now())
                .await;
            return Err(error);
        }
        Ok(())
    }

    async fn list_jobs(&self) -> Vec<JobListItem> {
        self.store
            .list_all()
            .await
            .into_iter()
            .map(|job| job.with_control_plane(JobControlPlaneInfo::temporal()))
            .collect()
    }

    async fn get_job(&self, job_id: &JobId) -> Option<JobInfo> {
        match self.store.get(job_id).await {
            Some(job) => Some(self.enrich_job_info(job).await),
            None => None,
        }
    }

    async fn get_job_detail(&self, job_id: &JobId) -> Option<JobDetail> {
        self.store.get_job_detail(job_id).await
    }

    async fn job_status(&self, job_id: &JobId) -> Option<JobStatus> {
        self.store.job_status(job_id).await
    }

    async fn is_job_running(&self, job_id: &JobId) -> Option<bool> {
        // Check the store first. If the store says the job is in a terminal
        // state (completed, cancelled, failed), return false immediately
        // without querying Temporal — the store is the source of truth for
        // job lifecycle.
        let store_status = self.store.job_status(job_id).await?;
        if store_status.is_terminal() {
            return Some(false);
        }
        if store_status == JobStatus::Running {
            return Some(true);
        }
        // Job is queued — check Temporal to see if a workflow is active.
        match self.describe_temporal_workflow(job_id).await {
            Ok(description) => Some(
                description
                    .raw_description
                    .workflow_execution_info
                    .as_ref()
                    .map(|info| temporal_workflow_status_is_active(info.status()))
                    .unwrap_or(false),
            ),
            Err(WorkflowInteractionError::NotFound(_)) => Some(false),
            Err(error) => {
                warn!(
                    job_id = %job_id,
                    error = %error,
                    "Temporal describe failed during running-state check; treating job as active"
                );
                Some(true)
            }
        }
    }

    async fn cancel_job(
        &self,
        job_id: &JobId,
        provenance: CancellationRequest,
    ) -> Result<(), ServerError> {
        self.store.cancel(job_id, provenance).await?;
        // Worker reap happens activity-side (see the heartbeat block
        // in `BatchalignTemporalActivities::run_job_attempt`); the
        // pool is reachable there but not here.
        self.cancel_temporal_workflow(job_id).await
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
        self.terminate_temporal_workflow(job_id, &format!("batchalign job {job_id} deleted"))
            .await?;
        self.store.delete(job_id).await
    }

    async fn restart_job(&self, job_id: &JobId) -> Result<JobInfo, ServerError> {
        let info = self.store.restart(job_id).await?;
        if let Err(error) = self
            .ensure_workflow_for_job(job_id, TemporalWorkflowStartMode::ReplaceExisting)
            .await
        {
            self.store
                .fail_job(job_id, &error.to_string(), unix_now())
                .await;
            return Err(error);
        }
        Ok(self.enrich_job_info(info).await)
    }

    async fn interrupt_all_for_shutdown(&self) -> usize {
        // Snapshot active job IDs before any mutations so the set is stable.
        let active_job_ids: Vec<JobId> = self
            .store
            .list_all()
            .await
            .into_iter()
            .filter(|job| job.status.can_cancel())
            .map(|job| job.job_id)
            .collect();

        // Step 1 — write forensic audit rows for every active job.
        //
        // These rows go into the `cancellations` table so operators can see which
        // jobs were mid-flight at shutdown time and distinguish them from jobs that
        // a user explicitly pressed Cancel on.  The `source = Signal` +
        // `reason = "server-cancel-all"` combination is a stable audit key that
        // Task 4's reconciler matches to suppress spurious "user cancelled" reconciler
        // actions on the next server start.
        //
        // This step deliberately does NOT change the local job lifecycle status —
        // that is step 2's job.  The reason for separating the two is the ordering
        // constraint explained below.
        let provenance = CancellationRequest {
            source: Some(CancelSource::Signal),
            reason: Some(CancelReason::server_cancel_all()),
            ..Default::default()
        };
        for job_id in &active_job_ids {
            self.store
                .record_cancellation_audit(job_id, &provenance)
                .await;
        }

        // Step 2 — flip the local lifecycle bit to `Interrupted` (resumable) BEFORE
        // sending Temporal workflow-cancel signals.
        //
        // Ordering invariant: `Interrupted` must be written to the local DB BEFORE
        // the Temporal-workflow cancel is sent so that the activity-side heartbeat
        // handler (temporal_backend.rs:287-308) can never observe the job as active
        // and overwrite it with `Cancelled`.  Here is why that matters:
        //
        //   * The activity's cancel-forward path calls `store.cancel()`, which calls
        //     `Job::request_cancellation()`, which gates on `can_cancel()`.
        //   * `can_cancel()` delegates to `is_active()`, which returns `false` for
        //     `JobStatus::Interrupted`.
        //   * Therefore, once step 2 has run, any concurrent `store.cancel()` triggered
        //     by Temporal's cancel signal is a no-op for lifecycle status (the audit row
        //     is still recorded, which is harmless).
        //
        // If we sent Temporal cancels FIRST (step 3 before step 2), there would be a
        // race window where Temporal's cancel arrives at the activity, fires
        // `store.cancel()`, writes `Cancelled`, and THEN step 2 runs — but
        // `interrupt_all_for_shutdown` skips the job because it's no longer active
        // (Cancelled is also not `can_cancel()`).  The job would stay `Cancelled`
        // forever, preventing recovery on the next server start.
        let interrupted = self.store.interrupt_all_for_shutdown().await;

        // Step 3 — cancel Temporal workflows so worker-pool activity tasks see
        // `ctx.is_cancelled()` and stop consuming ML resources cleanly.
        //
        // Errors here are best-effort: the worker will be stranded until the next
        // Temporal heartbeat timeout, but the local job is already `Interrupted` and
        // will be requeued by the startup recovery path.  A WARN-level log is
        // sufficient; we do not propagate errors or roll back the lifecycle bit.
        for job_id in &active_job_ids {
            if let Err(err) = self.cancel_temporal_workflow(job_id).await {
                warn!(
                    job_id = %job_id,
                    error = %err,
                    "Failed to cancel Temporal workflow during shutdown; \
                     job is Interrupted locally and will recover on next start"
                );
            }
        }

        interrupted
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
        self.worker_runtime.shutdown(timeout).await
    }
}

/// Build and start the Temporal-backed server control plane.
pub(crate) async fn bootstrap_temporal_server_backend(
    config: ServerConfig,
    db: Arc<JobDB>,
    engine: ExecutionEngine,
) -> Result<ServerBackendBootstrap, ServerError> {
    let (ws_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(config.clone(), Some(db), ws_tx.clone()));
    let loaded_jobs = store.load_from_db().await?;
    let queued_jobs = store
        .list_all()
        .await
        .into_iter()
        .filter(|job| job.status == JobStatus::Queued)
        .count();

    let connection = Connection::connect(
        ConnectionOptions::new(Url::parse(&config.temporal_server_url).map_err(|error| {
            ServerError::Validation(format!(
                "invalid temporal_server_url '{}': {error}",
                config.temporal_server_url
            ))
        })?)
        .identity("batchalign3-server-temporal".to_string())
        .build(),
    )
    .await
    .map_err(|error| {
        ServerError::Persistence(format!(
            "failed to connect to Temporal server at '{}': {error}. \
         Ensure the Temporal server is running and reachable.",
            config.temporal_server_url
        ))
    })?;
    let client = Client::new(
        connection,
        ClientOptions::new(config.temporal_namespace.clone()).build(),
    )
    .map_err(|error| {
        ServerError::Persistence(format!("failed to build Temporal client: {error}"))
    })?;

    let orchestrator = Arc::new(TemporalJobOrchestrator::new());
    let server_host = ServerExecutionHost::new(store.clone(), engine, orchestrator);
    // The RuntimeSupervisor is created here on the main server runtime.
    // Its actor loop runs on the main runtime's JoinSet, so job_task()
    // dispatched via spawn_job() will execute on the main runtime where
    // the JobStore actor is reachable. The completion oneshot returned by
    // spawn_job() crosses the runtime boundary to signal the Temporal
    // activity on the worker thread.
    let job_runtime = RuntimeSupervisor::new();
    let activities = BatchalignTemporalActivities {
        host: server_host,
        runtime: job_runtime,
    };
    let worker_runtime = TemporalWorkerRuntime::start(&config, client.clone(), activities)?;

    let backend_impl = Arc::new(TemporalServerBackend::new(
        store,
        client,
        worker_runtime,
        ws_tx,
        config,
    ));
    backend_impl.bootstrap_active_workflows().await?;

    // Background reconciler: bounded-staleness sync from Temporal's
    // authoritative workflow status into the local store.
    {
        let reconciler = backend_impl.reconciler();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(RECONCILER_TICK_S));
            // Skip the immediate first tick so we don't hammer Temporal
            // during startup while workflows are still registering.
            tick.tick().await;
            loop {
                tick.tick().await;
                let report = reconciler.reconcile_all_active().await;
                if report.updated > 0 || report.errored > 0 {
                    tracing::debug!(
                        updated = report.updated,
                        unchanged = report.unchanged,
                        errored = report.errored,
                        "Temporal reconciler tick"
                    );
                }
            }
        });
    }

    let backend: Arc<dyn ServerBackend> = backend_impl;

    info!(loaded_jobs, queued_jobs, "Temporal backend bootstrapped");

    Ok(ServerBackendBootstrap {
        backend,
        loaded_jobs,
        queued_jobs,
    })
}

#[cfg(test)]
fn retry_after_ms_from_retry_at(retry_at: UnixTimestamp) -> u64 {
    let now = unix_now().0;
    if retry_at.0 <= now {
        0
    } else {
        ((retry_at.0 - now) * 1000.0).ceil() as u64
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn temporal_workflow_status_name(status: WorkflowExecutionStatus) -> &'static str {
    match status {
        WorkflowExecutionStatus::Running => "running",
        WorkflowExecutionStatus::Completed => "completed",
        WorkflowExecutionStatus::Failed => "failed",
        WorkflowExecutionStatus::Canceled => "cancelled",
        WorkflowExecutionStatus::Terminated => "terminated",
        WorkflowExecutionStatus::ContinuedAsNew => "continued-as-new",
        WorkflowExecutionStatus::TimedOut => "timed-out",
        WorkflowExecutionStatus::Paused => "paused",
        WorkflowExecutionStatus::Unspecified => "unspecified",
    }
}

fn temporal_workflow_status_is_active(status: WorkflowExecutionStatus) -> bool {
    matches!(
        status,
        WorkflowExecutionStatus::Running | WorkflowExecutionStatus::Paused
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_ms_clamps_past_deadlines() {
        assert_eq!(
            retry_after_ms_from_retry_at(UnixTimestamp(unix_now().0 - 10.0)),
            0
        );
    }

    #[test]
    fn temporal_status_name_normalizes_proto_values() {
        assert_eq!(
            temporal_workflow_status_name(WorkflowExecutionStatus::Canceled),
            "cancelled"
        );
        assert_eq!(
            temporal_workflow_status_name(WorkflowExecutionStatus::ContinuedAsNew),
            "continued-as-new"
        );
    }

    #[test]
    fn temporal_active_status_matches_running_and_paused_only() {
        assert!(temporal_workflow_status_is_active(
            WorkflowExecutionStatus::Running
        ));
        assert!(temporal_workflow_status_is_active(
            WorkflowExecutionStatus::Paused
        ));
        assert!(!temporal_workflow_status_is_active(
            WorkflowExecutionStatus::Completed
        ));
    }

    #[test]
    fn non_empty_string_rejects_whitespace() {
        assert_eq!(non_empty_string(""), None);
        assert_eq!(non_empty_string("   "), None);
        assert_eq!(non_empty_string("run-123"), Some("run-123".into()));
    }
}
