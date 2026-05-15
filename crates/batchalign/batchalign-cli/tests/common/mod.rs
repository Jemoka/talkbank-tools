//! Shared live execution fixture support for real-model integration tests.
//!
//! The fixture owns one prepared worker pool on a dedicated background Tokio
//! runtime. Tests can then acquire either a fresh server session or a fresh
//! direct-execution session over that shared warmed backend. This keeps
//! expensive model loads warm across tests while preventing control-plane state
//! from bleeding between sessions.

#![allow(dead_code)]

mod audio_fixtures;
pub mod chat_fixtures;
mod direct_job_client;
pub mod drift_assertions;
pub mod drift_staging;
mod paths_mode;
pub mod pool_dispatch;
pub mod regression_manifest;
mod server_job_client;
pub mod test_server_fixture;
pub mod test_worker_pool;

#[allow(unused_imports)]
pub use audio_fixtures::{
    AudioFixtures, prepare_audio_fixtures, prepare_multi_speaker_audio, prepare_named_audio,
    strip_dependent_tiers,
};
#[allow(unused_imports)]
pub use direct_job_client::LiveDirectJobClient;
#[allow(unused_imports)]
pub use paths_mode::{
    submit_paths_and_complete, submit_paths_and_complete_direct,
    submit_paths_with_before_and_complete_direct,
};
#[allow(unused_imports)]
pub use server_job_client::LiveServerJobClient;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, mpsc};
use std::thread;
use std::time::Duration;

use batchalign::api::{
    FilePayload, FileResult, HealthResponse, JobInfo, JobListItem, JobResultResponse, JobStatus,
    JobSubmission, LanguageSpec, MemoryMb, NumSpeakers, ReleasedCommand, WorkerLanguage,
};
use batchalign::config::{RuntimeLayout, ServerConfig};
use batchalign::host_facts::PerProfile;
use batchalign::host_memory::MachineMlTestLock;
use batchalign::options::CommandOptions;
use batchalign::worker::InferTask;
use batchalign::worker::pool::PoolConfig;
use batchalign::{
    AppState, DirectHost, PreparedWorkers, create_app_with_prepared_workers, prepare_workers,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Cached worker backend that survives across isolated server and direct sessions.
struct LiveFixtureBackend {
    _machine_lock: MachineMlTestLock,
    prepared_workers: PreparedWorkers,
    session_config: ServerConfig,
}

impl LiveFixtureBackend {
    /// Build the shared worker backend used by live-model tests.
    async fn initialize() -> Result<Self, String> {
        let python_path = resolve_python()
            .ok_or_else(|| "Python 3 with batchalign is not available".to_string())?;
        let session_config = live_fixture_server_config();
        let machine_lock = MachineMlTestLock::acquire("batchalign live fixture")
            .map_err(|error| format!("machine-wide ML test lock unavailable: {error}"))?;
        let prepared_workers =
            prepare_workers(&session_config, live_fixture_pool_config(&python_path))
                .await
                .map_err(|error| format!("could not prepare live workers: {error}"))?;
        Ok(Self {
            _machine_lock: machine_lock,
            prepared_workers,
            session_config,
        })
    }
}

/// Shared worker-backend state for the fixture thread.
enum BackendState {
    /// No backend has been prepared yet.
    Uninitialized,
    /// Prepared workers are ready for reuse.
    Ready(Box<LiveFixtureBackend>),
    /// Backend initialization failed and later callers should skip quickly.
    Unavailable(String),
}

/// One active server session running on the dedicated fixture runtime.
struct ActiveSession {
    state: Arc<AppState>,
    runtime_root: tempfile::TempDir,
    server_task: tokio::task::JoinHandle<()>,
}

/// Immutable session metadata returned to test code.
#[derive(Clone)]
struct SessionSnapshot {
    base_url: String,
    state_dir: PathBuf,
    infer_tasks: Vec<InferTask>,
}

/// Prepared direct-execution metadata returned to test code.
#[derive(Clone)]
struct DirectSnapshot {
    prepared_workers: PreparedWorkers,
    infer_tasks: Vec<InferTask>,
}

/// Optional direct-fixture pre-scale request.
#[derive(Clone)]
struct DirectWarmupRequest {
    command: ReleasedCommand,
    lang: WorkerLanguage,
}

/// Commands sent from tests into the dedicated fixture thread.
enum FixtureCommand {
    /// Start one isolated server session backed by the shared prepared workers.
    Acquire {
        /// Optional command/lang pairs to pre-scale on the fixture runtime
        /// before handing the server snapshot back to the test runtime.
        warmups: Vec<DirectWarmupRequest>,
        /// Synchronous reply channel for session metadata or skip reasons.
        reply: mpsc::Sender<Result<SessionSnapshot, String>>,
    },
    /// Return a clone of the shared warmed worker backend for direct execution.
    AcquireDirect {
        /// Optional command/lang pairs to pre-scale on the fixture runtime
        /// before handing the direct snapshot back to the test runtime.
        warmups: Vec<DirectWarmupRequest>,
        /// Synchronous reply channel for prepared workers or skip reasons.
        reply: mpsc::Sender<Result<DirectSnapshot, String>>,
    },
    /// Tear down the currently active isolated session.
    Release {
        /// Synchronous ack channel completed after teardown finishes.
        reply: mpsc::Sender<()>,
    },
}

/// Bridge that lets test threads talk to the fixture thread.
struct FixtureBridge {
    commands: mpsc::Sender<FixtureCommand>,
    /// Serialize server and direct sessions over one warmed backend.
    session_slots: Arc<Semaphore>,
}

/// Global bridge for the dedicated live-fixture runtime thread.
static LIVE_FIXTURE: LazyLock<Arc<FixtureBridge>> = LazyLock::new(start_fixture_thread);

/// Handle to one isolated live-server session backed by shared warmed workers.
pub struct LiveServerSession {
    base_url: String,
    client: reqwest::Client,
    state_dir: PathBuf,
    infer_tasks: Vec<InferTask>,
    slot: Option<OwnedSemaphorePermit>,
    bridge: Arc<FixtureBridge>,
}

/// Handle to one isolated direct-execution session backed by shared warmed workers.
pub struct LiveDirectSession {
    host: DirectHost,
    state_dir: PathBuf,
    infer_tasks: Vec<InferTask>,
    _runtime_root: tempfile::TempDir,
    _slot: OwnedSemaphorePermit,
}

impl LiveServerSession {
    /// Acquire an isolated live-model server session.
    ///
    /// The worker backend is prepared once on a dedicated background runtime.
    /// Each call then creates a fresh runtime layout rooted in a new temp dir so
    /// jobs, SQLite state, cache state, and the runtime supervisor do not bleed
    /// into the next session.
    pub async fn acquire() -> Option<Self> {
        Self::acquire_with_warmups(Vec::new()).await
    }

    /// Acquire an isolated live-model server session and optionally pre-scale
    /// the requested command/lang pairs on the fixture runtime first.
    pub async fn acquire_with_warmups(warmups: Vec<(ReleasedCommand, &str)>) -> Option<Self> {
        let bridge = LIVE_FIXTURE.clone();
        let slot = bridge
            .session_slots
            .clone()
            .acquire_owned()
            .await
            .expect("live fixture semaphore should stay open");
        let bridge_for_request = bridge.clone();
        let warmups = warmups
            .into_iter()
            .map(|(command, lang)| DirectWarmupRequest {
                command,
                lang: WorkerLanguage::try_from(lang).expect("test warmup lang must be valid"),
            })
            .collect();
        let snapshot = tokio::task::spawn_blocking(move || {
            request_session_snapshot(&bridge_for_request, warmups)
        })
        .await
        .expect("live fixture acquire task should not panic");
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(message) => {
                eprintln!("SKIP: {message}");
                return None;
            }
        };

        Some(Self {
            base_url: snapshot.base_url,
            client: reqwest::Client::new(),
            state_dir: snapshot.state_dir,
            infer_tasks: snapshot.infer_tasks,
            slot: Some(slot),
            bridge,
        })
    }

    /// HTTP base URL for the isolated server instance.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Shared HTTP client for the session.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Runtime-owned state directory for this isolated session.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Return true when the prepared worker backend advertises one infer task.
    pub fn has_infer_task(&self, task: InferTask) -> bool {
        self.infer_tasks.contains(&task)
    }

    /// Read the session's health snapshot.
    pub async fn health(&self) -> HealthResponse {
        self.client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .expect("GET /health")
            .json()
            .await
            .expect("parse health")
    }

    /// List jobs currently visible to this isolated server session.
    pub async fn list_jobs(&self) -> Vec<JobListItem> {
        self.client
            .get(format!("{}/jobs", self.base_url))
            .send()
            .await
            .expect("GET /jobs")
            .json()
            .await
            .expect("parse jobs")
    }

    /// Shut down the isolated session deterministically.
    pub async fn close(mut self) {
        if let Some((bridge, slot)) = self.begin_release() {
            tokio::task::spawn_blocking(move || {
                let _ = release_active_session(&bridge);
                drop(slot);
            })
            .await
            .expect("live fixture release task should not panic");
        }
    }

    /// Take the release inputs so `close()` and `Drop` share one path.
    fn begin_release(&mut self) -> Option<(Arc<FixtureBridge>, OwnedSemaphorePermit)> {
        Some((self.bridge.clone(), self.slot.take()?))
    }
}

impl Drop for LiveServerSession {
    fn drop(&mut self) {
        let Some((bridge, slot)) = self.begin_release() else {
            return;
        };

        thread::spawn(move || {
            let _ = release_active_session(&bridge);
            drop(slot);
        });
    }
}

impl LiveDirectSession {
    /// Acquire one isolated direct-execution session.
    pub async fn acquire() -> Option<Self> {
        Self::acquire_with_warmups(Vec::new()).await
    }

    /// Acquire one isolated direct-execution session and optionally pre-scale
    /// the requested command/lang pairs on the fixture runtime first.
    pub async fn acquire_with_warmups(warmups: Vec<(ReleasedCommand, &str)>) -> Option<Self> {
        let bridge = LIVE_FIXTURE.clone();
        let slot = bridge
            .session_slots
            .clone()
            .acquire_owned()
            .await
            .expect("live fixture semaphore should stay open");
        let bridge_for_request = bridge.clone();
        let warmups = warmups
            .into_iter()
            .map(|(command, lang)| DirectWarmupRequest {
                command,
                lang: WorkerLanguage::try_from(lang).expect("test warmup lang must be valid"),
            })
            .collect();
        let snapshot = tokio::task::spawn_blocking(move || {
            request_direct_snapshot(&bridge_for_request, warmups)
        })
        .await
        .expect("live direct fixture acquire task should not panic");
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(message) => {
                eprintln!("SKIP: {message}");
                return None;
            }
        };

        let runtime_root = tempfile::TempDir::new().expect("tempdir");
        let state_dir = runtime_root.path().to_path_buf();
        let host = DirectHost::new(
            live_fixture_server_config(),
            RuntimeLayout::from_state_dir(state_dir.clone()),
            None,
            Some(state_dir.join("cache")),
            &snapshot.prepared_workers,
        )
        .await
        .expect("create live direct host");

        Some(Self {
            host,
            state_dir,
            infer_tasks: snapshot.infer_tasks,
            _runtime_root: runtime_root,
            _slot: slot,
        })
    }

    /// Runtime-owned state directory for this isolated direct session.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Return true when the shared warmed worker backend advertises one infer task.
    pub fn has_infer_task(&self, task: InferTask) -> bool {
        self.infer_tasks.contains(&task)
    }

    /// Run one submission inline and return the final job projections.
    pub async fn run_submission(
        &self,
        submission: JobSubmission,
    ) -> (JobInfo, batchalign::store::JobDetail) {
        let outcome = self
            .host
            .run_submission(submission)
            .await
            .expect("run direct submission");
        (outcome.info, outcome.detail)
    }
}

/// Poll one submitted job until it reaches a terminal state.
pub async fn poll_job_done(client: &reqwest::Client, base_url: &str, job_id: &str) -> JobInfo {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(300);

    loop {
        let resp = client
            .get(format!("{base_url}/jobs/{job_id}"))
            .send()
            .await
            .expect("GET /jobs/{job_id}");
        let info: JobInfo = resp.json().await.expect("parse job");

        if matches!(
            info.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return info;
        }

        assert!(
            tokio::time::Instant::now() < deadline,
            "Job {job_id} did not finish within 5 min (status: {:?})",
            info.status
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

/// Acquire one isolated live-server session and skip cleanly when a task is unavailable.
pub async fn require_live_server(task: InferTask, skip_message: &str) -> Option<LiveServerSession> {
    let server = LiveServerSession::acquire().await?;
    if !server.has_infer_task(task) {
        eprintln!("SKIP: {skip_message}");
        return None;
    }
    Some(server)
}

/// Acquire one isolated live-server session and pre-scale the requested
/// command/lang on the fixture runtime before the test uses it.
pub async fn require_live_server_warmed(
    task: InferTask,
    command: ReleasedCommand,
    lang: &str,
    skip_message: &str,
) -> Option<LiveServerSession> {
    let server = LiveServerSession::acquire_with_warmups(vec![(command, lang)]).await?;
    if !server.has_infer_task(task) {
        eprintln!("SKIP: {skip_message}");
        return None;
    }
    Some(server)
}

/// Acquire one isolated live direct-execution session and skip cleanly when a task is unavailable.
pub async fn require_live_direct(task: InferTask, skip_message: &str) -> Option<LiveDirectSession> {
    let session = LiveDirectSession::acquire().await?;
    if !session.has_infer_task(task) {
        eprintln!("SKIP: {skip_message}");
        return None;
    }
    Some(session)
}

/// Acquire one isolated live direct-execution session and pre-scale the
/// requested command/lang on the fixture runtime before the test uses it.
pub async fn require_live_direct_warmed(
    task: InferTask,
    command: ReleasedCommand,
    lang: &str,
    skip_message: &str,
) -> Option<LiveDirectSession> {
    let session = LiveDirectSession::acquire_with_warmups(vec![(command, lang)]).await?;
    if !session.has_infer_task(task) {
        eprintln!("SKIP: {skip_message}");
        return None;
    }
    Some(session)
}

/// Acquire one isolated live direct-execution session and pre-scale multiple
/// command/lang pairs on the fixture runtime before the test uses it.
pub async fn require_live_direct_warmed_many(
    task: InferTask,
    warmups: Vec<(ReleasedCommand, &str)>,
    skip_message: &str,
) -> Option<LiveDirectSession> {
    let session = LiveDirectSession::acquire_with_warmups(warmups).await?;
    if !session.has_infer_task(task) {
        eprintln!("SKIP: {skip_message}");
        return None;
    }
    Some(session)
}

async fn collect_direct_content_results(detail: &batchalign::store::JobDetail) -> Vec<FileResult> {
    if detail.paths_mode {
        return detail
            .results
            .iter()
            .map(|result| FileResult {
                filename: result.filename.clone(),
                content: String::new(),
                content_type: result.content_type,
                error: result.error.clone(),
                provenance: Vec::new(),
            })
            .collect();
    }

    let output_dir = detail.staging_dir.as_path().join("output");
    let mut files = Vec::new();
    for result in &detail.results {
        let content = if result.error.is_none() {
            let path = output_dir.join(&*result.filename);
            tokio::fs::read_to_string(&path)
                .await
                .unwrap_or_else(|error| panic!("read direct result {}: {error}", path.display()))
        } else {
            String::new()
        };
        files.push(FileResult {
            filename: result.filename.clone(),
            content,
            content_type: result.content_type,
            error: result.error.clone(),
            provenance: Vec::new(),
        });
    }
    files
}

/// Submit one content-mode job to a live server and return the completed results.
pub async fn submit_and_complete(
    client: &reqwest::Client,
    base_url: &str,
    command: ReleasedCommand,
    lang: &str,
    files: Vec<FilePayload>,
    options: CommandOptions,
) -> (JobInfo, Vec<FileResult>) {
    let submission = JobSubmission {
        command,
        lang: LanguageSpec::try_from(lang)
            .expect("test lang must be a valid ISO 639-3 code or \"auto\""),
        num_speakers: NumSpeakers(1),
        files,
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200, "Job submission should succeed");
    let info: JobInfo = resp.json().await.expect("parse initial JobInfo");

    let final_info = poll_job_done(client, base_url, &info.job_id).await;

    let results: JobResultResponse = client
        .get(format!("{base_url}/jobs/{}/results", info.job_id))
        .send()
        .await
        .expect("GET results")
        .json()
        .await
        .expect("parse results");

    (final_info, results.files)
}

/// Submit one content-mode job to a live direct session and return the completed results.
pub async fn submit_and_complete_direct(
    session: &LiveDirectSession,
    command: ReleasedCommand,
    lang: &str,
    files: Vec<FilePayload>,
    options: CommandOptions,
) -> (JobInfo, Vec<FileResult>) {
    let submission = JobSubmission {
        command,
        lang: LanguageSpec::try_from(lang)
            .expect("test lang must be a valid ISO 639-3 code or \"auto\""),
        num_speakers: NumSpeakers(1),
        files,
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let (info, detail) = session.run_submission(submission).await;
    let results = collect_direct_content_results(&detail).await;
    (info, results)
}

/// Assert a live-server job completed cleanly without per-file failures.
pub fn assert_completed_without_errors(label: &str, info: &JobInfo, results: &[FileResult]) {
    assert_eq!(
        info.status,
        JobStatus::Completed,
        "{label} should complete successfully; results={results:#?}"
    );
    assert!(
        results.iter().all(|result| result.error.is_none()),
        "{label} should not report per-file errors; results={results:#?}"
    );
}

/// Resolve the Python path for tests. Prefers the project venv over `python3`.
pub fn resolve_python_for_module(module: &str) -> Option<String> {
    if let Ok(dir) = std::env::current_dir() {
        let mut cursor = dir;
        loop {
            for venv in preferred_venv_pythons(&cursor) {
                if venv.exists() && python_imports_module(&venv, module) {
                    return Some(venv.to_string_lossy().to_string());
                }
            }
            if !cursor.pop() {
                break;
            }
        }
    }

    for candidate in preferred_path_pythons() {
        if python_imports_module(candidate, module) {
            return Some((*candidate).to_string());
        }
    }

    None
}

pub fn resolve_python() -> Option<String> {
    resolve_python_for_module("batchalign.worker")
}

fn python_imports_module(command: impl AsRef<std::ffi::OsStr>, module: &str) -> bool {
    let snippet = format!("import {module}");
    std::process::Command::new(command)
        .args(["-c", &snippet])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn preferred_venv_pythons(root: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![root.join(".venv").join("Scripts").join("python.exe")]
    } else {
        let bin = root.join(".venv").join("bin");
        vec![
            bin.join("python3.12"),
            bin.join("python3"),
            bin.join("python"),
        ]
    }
}

fn preferred_path_pythons() -> &'static [&'static str] {
    if cfg!(windows) {
        &["python"]
    } else {
        &["python3.12", "python3"]
    }
}

/// Start the dedicated fixture thread and return the test-side bridge.
fn start_fixture_thread() -> Arc<FixtureBridge> {
    let (commands, receiver) = mpsc::channel();
    let bridge = Arc::new(FixtureBridge {
        commands,
        session_slots: Arc::new(Semaphore::new(1)),
    });

    thread::Builder::new()
        .name("batchalign-live-fixture".into())
        .spawn(move || run_fixture_thread(receiver))
        .expect("live fixture thread should spawn");

    bridge
}

/// Run the dedicated fixture runtime thread.
fn run_fixture_thread(receiver: mpsc::Receiver<FixtureCommand>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("live fixture runtime should build");
    let mut backend = BackendState::Uninitialized;
    let mut active_session: Option<ActiveSession> = None;

    while let Ok(command) = receiver.recv() {
        match command {
            FixtureCommand::Acquire { warmups, reply } => {
                if active_session.is_some() {
                    let _ = reply.send(Err(
                        "live fixture session already active while acquiring a new one".to_string(),
                    ));
                    continue;
                }

                let backend = match ensure_backend(&runtime, &mut backend) {
                    Ok(backend) => backend,
                    Err(message) => {
                        let _ = reply.send(Err(message));
                        continue;
                    }
                };

                for warmup in warmups {
                    runtime.block_on(async {
                        backend
                            .prepared_workers
                            .pool()
                            .pre_scale(warmup.command, warmup.lang, 1)
                            .await;
                    });
                }

                match runtime.block_on(start_session(backend)) {
                    Ok((session, snapshot)) => {
                        active_session = Some(session);
                        let _ = reply.send(Ok(snapshot));
                    }
                    Err(message) => {
                        let _ = reply.send(Err(message));
                    }
                }
            }
            FixtureCommand::AcquireDirect { warmups, reply } => {
                if active_session.is_some() {
                    let _ = reply.send(Err(
                        "live fixture server session already active while acquiring a direct session"
                            .to_string(),
                    ));
                    continue;
                }

                let backend = match ensure_backend(&runtime, &mut backend) {
                    Ok(backend) => backend,
                    Err(message) => {
                        let _ = reply.send(Err(message));
                        continue;
                    }
                };

                for warmup in warmups {
                    runtime.block_on(async {
                        backend
                            .prepared_workers
                            .pool()
                            .pre_scale(warmup.command, warmup.lang, 1)
                            .await;
                    });
                }

                let snapshot = DirectSnapshot {
                    prepared_workers: backend.prepared_workers.clone(),
                    infer_tasks: backend
                        .prepared_workers
                        .current_infer_tasks()
                        .unwrap_or_else(|_| backend.prepared_workers.infer_tasks().to_vec()),
                };
                let _ = reply.send(Ok(snapshot));
            }
            FixtureCommand::Release { reply } => {
                if let Some(session) = active_session.take() {
                    runtime.block_on(cleanup_session(session));
                }
                let _ = reply.send(());
            }
        }
    }

    if let Some(session) = active_session.take() {
        runtime.block_on(cleanup_session(session));
    }
}

/// Prepare the backend on first use and cache success or failure.
fn ensure_backend<'a>(
    runtime: &tokio::runtime::Runtime,
    state: &'a mut BackendState,
) -> Result<&'a LiveFixtureBackend, String> {
    if matches!(state, BackendState::Uninitialized) {
        *state = match runtime.block_on(LiveFixtureBackend::initialize()) {
            Ok(backend) => BackendState::Ready(Box::new(backend)),
            Err(message) => BackendState::Unavailable(message),
        };
    }

    match state {
        BackendState::Ready(backend) => Ok(backend),
        BackendState::Unavailable(message) => Err(message.clone()),
        BackendState::Uninitialized => unreachable!("backend should be initialized before use"),
    }
}

/// Start one isolated app/server session on the dedicated fixture runtime.
async fn start_session(
    backend: &LiveFixtureBackend,
) -> Result<(ActiveSession, SessionSnapshot), String> {
    let runtime_root = tempfile::TempDir::new().expect("tempdir");
    let layout = RuntimeLayout::from_state_dir(runtime_root.path().to_path_buf());
    let cache_dir = runtime_root.path().join("cache");
    let (router, state) = create_app_with_prepared_workers(
        backend.session_config.clone(),
        layout,
        None,
        None,
        Some(cache_dir),
        Some("live-fixture-hash".into()),
        backend.prepared_workers.clone(),
    )
    .await
    .map_err(|error| format!("Could not create app with live fixture workers: {error}"))?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    let base_url = format!("http://127.0.0.1:{port}");
    let server_task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let snapshot = SessionSnapshot {
        base_url,
        state_dir: runtime_root.path().to_path_buf(),
        infer_tasks: state.infer_tasks().to_vec(),
    };
    let session = ActiveSession {
        state,
        runtime_root,
        server_task,
    };
    Ok((session, snapshot))
}

/// Tear down the active session on the dedicated fixture runtime.
async fn cleanup_session(session: ActiveSession) {
    let ActiveSession {
        state,
        runtime_root,
        server_task,
    } = session;
    cleanup_active_session(state, runtime_root, server_task, "live fixture").await;
}

/// Shared session-teardown for both the ML live fixture and the
/// test-echo fixture: aborts the axum server task, drains its handle,
/// runs `shutdown_for_reuse` with a 5s budget, drops `state` before
/// yielding so background actors observe the drop, and then drops the
/// runtime tempdir last so per-session state outlives any task that
/// might still touch it. `label` is used only in WARN log lines so
/// operators can tell which fixture failed to shut down cleanly.
pub(super) async fn cleanup_active_session(
    state: Arc<AppState>,
    runtime_root: tempfile::TempDir,
    server_task: tokio::task::JoinHandle<()>,
    label: &'static str,
) {
    server_task.abort();
    let _ = server_task.await;
    match state.shutdown_for_reuse(Duration::from_secs(5)).await {
        Ok(shutdown) if shutdown.timed_out || shutdown.remaining_jobs > 0 => {
            eprintln!(
                "WARN: {label} shutdown left {} tracked jobs (timed_out={})",
                shutdown.remaining_jobs, shutdown.timed_out
            );
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!("WARN: {label} shutdown failed to report runtime status: {error}");
        }
    }
    drop(state);
    tokio::task::yield_now().await;
    drop(runtime_root);
}

/// Request one session snapshot from the fixture thread.
fn request_session_snapshot(
    bridge: &Arc<FixtureBridge>,
    warmups: Vec<DirectWarmupRequest>,
) -> Result<SessionSnapshot, String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    bridge
        .commands
        .send(FixtureCommand::Acquire {
            warmups,
            reply: reply_tx,
        })
        .map_err(|error| format!("live fixture acquire send failed: {error}"))?;
    reply_rx
        .recv()
        .map_err(|error| format!("live fixture acquire recv failed: {error}"))?
}

/// Request one direct-execution snapshot from the fixture thread.
fn request_direct_snapshot(
    bridge: &Arc<FixtureBridge>,
    warmups: Vec<DirectWarmupRequest>,
) -> Result<DirectSnapshot, String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    bridge
        .commands
        .send(FixtureCommand::AcquireDirect {
            warmups,
            reply: reply_tx,
        })
        .map_err(|error| format!("live direct fixture acquire send failed: {error}"))?;
    reply_rx
        .recv()
        .map_err(|error| format!("live direct fixture acquire recv failed: {error}"))?
}

/// Release the active session and wait for teardown to finish.
fn release_active_session(bridge: &Arc<FixtureBridge>) -> Result<(), String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    bridge
        .commands
        .send(FixtureCommand::Release { reply: reply_tx })
        .map_err(|error| format!("live fixture release send failed: {error}"))?;
    reply_rx
        .recv()
        .map_err(|error| format!("live fixture release recv failed: {error}"))?;
    Ok(())
}

/// Walk upward from cwd to find the repo root (directory containing `Cargo.toml`
/// with `[workspace]` or the `batchalign/` directory).
pub(crate) fn find_repo_root() -> Option<PathBuf> {
    let mut cursor = std::env::current_dir().ok()?;
    loop {
        if cursor.join("batchalign").is_dir() && cursor.join("Cargo.toml").is_file() {
            return Some(cursor);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

/// Read the Rev.AI API key from environment variables.
///
/// Checks environment variables first, then the legacy `~/.batchalign.ini`
/// path the production server also honors.
pub fn require_revai_key() -> Option<String> {
    std::env::var("REVAI_API_KEY")
        .ok()
        .or_else(|| std::env::var("BATCHALIGN_REV_API_KEY").ok())
        .filter(|k| !k.is_empty())
        .or_else(load_legacy_revai_key)
}

fn load_legacy_revai_key() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let contents = std::fs::read_to_string(PathBuf::from(home).join(".batchalign.ini")).ok()?;
    parse_legacy_revai_key(&contents)
}

fn parse_legacy_revai_key(contents: &str) -> Option<String> {
    let mut current_section = String::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase();
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };

        if current_section == "asr" && key.trim() == "engine.rev.key" {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// BA2 parity helpers
// ---------------------------------------------------------------------------

/// Load a CHAT fixture from `python/batchalign/tests/support/parity/{name}.cha`.
///
/// Returns `None` if the fixture file doesn't exist (test should skip).
pub fn load_parity_fixture(name: &str) -> Option<String> {
    let repo_root = find_repo_root()?;
    let path = repo_root.join(format!("python/batchalign/tests/support/parity/{name}.cha"));
    if !path.exists() {
        eprintln!("SKIP: parity fixture not found: {}", path.display());
        return None;
    }
    Some(
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read parity fixture {}: {e}", path.display())),
    )
}

/// Load a BA2 Jan 9 golden reference output.
///
/// Reads from `batchalign/tests/golden/ba2_reference/{command}/{name}.jan9.cha`.
/// Returns `None` if not yet generated (parity test should still run with
/// structural assertions only).
pub fn load_ba2_golden(command: &str, name: &str) -> Option<String> {
    let repo_root = find_repo_root()?;
    let path = repo_root.join(format!(
        "batchalign/tests/golden/ba2_reference/{command}/{name}.jan9.cha"
    ));
    if !path.exists() {
        eprintln!(
            "NOTE: BA2 golden not found (run scripts/generate_ba2_golden.sh): {}",
            path.display()
        );
        return None;
    }
    Some(
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read BA2 golden {}: {e}", path.display())),
    )
}

/// Load a compare fixture pair (`FILE.cha` plus `FILE.gold.cha`).
///
/// Returns `None` if either companion file does not exist.
pub fn load_compare_fixture_pair(name: &str) -> Option<(String, String)> {
    let repo_root = find_repo_root()?;
    let main_path = repo_root.join(format!("python/batchalign/tests/support/parity/{name}.cha"));
    let gold_path = repo_root.join(format!("python/batchalign/tests/support/parity/{name}.gold.cha"));
    if !main_path.exists() || !gold_path.exists() {
        eprintln!(
            "SKIP: compare fixture pair not found: {} / {}",
            main_path.display(),
            gold_path.display()
        );
        return None;
    }
    let main = std::fs::read_to_string(&main_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read compare fixture {}: {e}",
            main_path.display()
        )
    });
    let gold = std::fs::read_to_string(&gold_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read compare fixture {}: {e}",
            gold_path.display()
        )
    });
    Some((main, gold))
}

/// Load committed batchalign2-master compare golden outputs.
///
/// Reads from `batchalign/tests/golden/ba2_reference/compare/{name}.master.cha`
/// plus the companion `.master.compare.csv`.
pub fn load_ba2_compare_master_golden(name: &str) -> Option<(String, String)> {
    let repo_root = find_repo_root()?;
    let chat_path = repo_root.join(format!(
        "batchalign/tests/golden/ba2_reference/compare/{name}.master.cha"
    ));
    let csv_path = repo_root.join(format!(
        "batchalign/tests/golden/ba2_reference/compare/{name}.master.compare.csv"
    ));
    if !chat_path.exists() || !csv_path.exists() {
        eprintln!(
            "SKIP: BA2 master compare golden not found: {} / {}",
            chat_path.display(),
            csv_path.display()
        );
        return None;
    }
    let chat = std::fs::read_to_string(&chat_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read BA2 master compare golden {}: {e}",
            chat_path.display()
        )
    });
    let csv = std::fs::read_to_string(&csv_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read BA2 master compare golden {}: {e}",
            csv_path.display()
        )
    });
    Some((chat, csv))
}

/// Compare BA3 output to BA2 golden reference, ignoring metadata differences.
///
/// Filters out lines that are expected to differ between BA2 and BA3
/// (timestamps, PID headers, tool-specific comments) and compares the
/// remaining content line-by-line.
///
/// This deliberately uses line-level text comparison rather than AST diffing
/// because the purpose is to verify textual parity with batchalign2-master
/// output. AST structural equality would miss formatting and ordering
/// differences that matter for CHAT file compatibility.
///
/// Panics with a detailed diff on mismatch.
pub fn assert_ba2_parity(label: &str, ba3_output: &str, ba2_golden: &str) {
    let normalize = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|line| {
                // Skip lines that naturally differ between BA2 and BA3
                !line.starts_with("@PID:")
                    && !line.starts_with("@Date:")
                    && !line.starts_with("@Comment:\t@Languages")
                    && !line.starts_with("@Tape Location:")
                    && !line.starts_with("@New Episode")
                    && !line.starts_with("@Situation:")
                    // Participant/ID ordering may differ — skip for comparison
                    && !line.starts_with("@Participants:")
                    && !line.starts_with("@ID:")
            })
            .map(|line| {
                let mut l = line.trim_end().to_string();
                // Normalize %gra ROOT convention: BA2 uses N|ROOT (self-ref),
                // BA3 uses 0|ROOT (UD standard). Convert BA2 to BA3 convention.
                if l.starts_with("%gra:") {
                    l = normalize_gra_root(&l);
                }
                l
            })
            .collect()
    };

    let ba3_lines = normalize(ba3_output);
    let ba2_lines = normalize(ba2_golden);

    if ba3_lines == ba2_lines {
        return;
    }

    // Build a useful diff report
    let mut report = format!("\n=== BA2 PARITY FAILURE: {label} ===\n\n");

    let max_lines = ba3_lines.len().max(ba2_lines.len());
    let mut diff_count = 0;
    for i in 0..max_lines {
        let ba3_line = ba3_lines.get(i).map(|s| s.as_str()).unwrap_or("<missing>");
        let ba2_line = ba2_lines.get(i).map(|s| s.as_str()).unwrap_or("<missing>");

        if ba3_line != ba2_line {
            diff_count += 1;
            report.push_str(&format!(
                "Line {i}:\n  BA2: {ba2_line}\n  BA3: {ba3_line}\n\n"
            ));
            if diff_count >= 20 {
                report.push_str("  ... (truncated, too many diffs)\n");
                break;
            }
        }
    }

    report.push_str(&format!(
        "Total lines: BA2={}, BA3={}, diffs={diff_count}\n",
        ba2_lines.len(),
        ba3_lines.len(),
    ));

    panic!("{report}");
}

/// Compare two text artifacts exactly after normalizing line endings and
/// trimming line-end whitespace.
pub fn assert_exact_text_parity(label: &str, actual: &str, expected: &str) {
    let normalize =
        |s: &str| -> Vec<String> { s.lines().map(|line| line.trim_end().to_string()).collect() };

    let actual_lines = normalize(actual);
    let expected_lines = normalize(expected);
    if actual_lines == expected_lines {
        return;
    }

    let mut report = format!("\n=== EXACT PARITY FAILURE: {label} ===\n\n");
    let max_lines = actual_lines.len().max(expected_lines.len());
    let mut diff_count = 0;
    for i in 0..max_lines {
        let actual_line = actual_lines
            .get(i)
            .map(|s| s.as_str())
            .unwrap_or("<missing>");
        let expected_line = expected_lines
            .get(i)
            .map(|s| s.as_str())
            .unwrap_or("<missing>");
        if actual_line != expected_line {
            diff_count += 1;
            report.push_str(&format!(
                "Line {i}:\n  expected: {expected_line}\n  actual:   {actual_line}\n\n"
            ));
            if diff_count >= 20 {
                report.push_str("  ... (truncated, too many diffs)\n");
                break;
            }
        }
    }

    report.push_str(&format!(
        "Total lines: expected={}, actual={}, diffs={diff_count}\n",
        expected_lines.len(),
        actual_lines.len(),
    ));
    panic!("{report}");
}

/// Normalize %gra ROOT convention: convert BA2's self-referencing ROOT
/// (e.g., `4|7|ROOT` where 7 is itself) to BA3's UD-standard `4|0|ROOT`.
fn normalize_gra_root(gra_line: &str) -> String {
    // %gra lines contain space-separated items like "1|2|NSUBJ 2|0|ROOT 3|2|PUNCT"
    // Find the ROOT item and set its head to 0.
    let prefix = if gra_line.starts_with("%gra:\t") {
        "%gra:\t"
    } else if gra_line.starts_with("%gra:") {
        "%gra:"
    } else {
        return gra_line.to_string();
    };

    let items: Vec<&str> = gra_line[prefix.len()..].split(' ').collect();
    let normalized: Vec<String> = items
        .iter()
        .map(|item| {
            if item.ends_with("|ROOT") {
                // Replace N|M|ROOT with N|0|ROOT
                let parts: Vec<&str> = item.splitn(3, '|').collect();
                if parts.len() == 3 {
                    format!("{}|0|ROOT", parts[0])
                } else {
                    item.to_string()
                }
            } else {
                item.to_string()
            }
        })
        .collect();

    format!("{prefix}{}", normalized.join(" "))
}

/// Real-model server config for the live fixture.
fn live_fixture_server_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".into(),
        port: 0,
        job_ttl_days: 1,
        warmup_commands: vec!["morphotag".into()],
        memory_gate_mb: Some(MemoryMb(0)),
        ..Default::default()
    }
}

/// Worker-pool config tuned for live-model fixture reuse.
///
/// Key memory safety settings:
///   to prevent memory accumulation when tests cycle through ASR→FA→Speaker→OpenSMILE.
///   On a 64GB machine, keeping all task workers resident simultaneously can OOM.
/// - `max_workers_per_key: 1` — one worker per (task, lang) pair. Tests are
///   serialized via semaphore anyway, so >1 just wastes memory.
fn live_fixture_pool_config(python_path: &str) -> PoolConfig {
    PoolConfig {
        python_path: python_path.into(),
        test_echo: false,
        health_check_interval_s: 3_600,
        ready_timeout_s: 120,
        // Allow 2 workers per key so sequential tests don't block waiting
        // for a prior test's checked-out worker to be returned to the pool.
        max_workers_per_key: PerProfile::uniform(2),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_legacy_revai_key;

    #[test]
    fn parse_legacy_revai_key_reads_asr_section_value() {
        let contents = "[asr]\nengine = rev\nengine.rev.key = secret\n";
        assert_eq!(parse_legacy_revai_key(contents).as_deref(), Some("secret"));
    }

    #[test]
    fn parse_legacy_revai_key_ignores_other_sections() {
        let contents = "[ud]\nengine.rev.key = nope\n[asr]\nengine = whisper\n";
        assert_eq!(parse_legacy_revai_key(contents), None);
    }
}
