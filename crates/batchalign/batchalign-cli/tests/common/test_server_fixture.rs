//! Shared test-echo server fixture.
//!
//! Mirrors the structure of [`super`]'s `LiveFixtureBackend` but for
//! `--test-echo` workers (no real models, no GPU/memory contention, no
//! `MachineMlTestLock`). The goal is to prepare one
//! [`PreparedWorkers`] per test-binary process and share it across
//! every test that today calls a bespoke `start_test_server` helper.
//!
//! Each test still gets a fresh axum server instance, jobs/cache
//! directories, and SQLite state — only the worker pool is shared.
//!
//! Why a dedicated runtime thread: the prepared workers and their
//! axum servers must outlive any individual test's tokio runtime.
//! Each `#[tokio::test]` builds and tears down its own runtime; tasks
//! pinned to that runtime would die with it. A long-lived background
//! thread with its own multi-threaded runtime owns the workers and
//! per-session servers; tests communicate via `mpsc` and HTTP.
//!
//! Unlike the live ML fixture, sessions here are NOT singleton —
//! tests may run concurrently against the same warmed pool.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, mpsc};
use std::thread;
use std::time::Duration;

use batchalign::api::MemoryMb;
use batchalign::config::{RuntimeLayout, ServerConfig};
use batchalign::worker::pool::PoolConfig;
use batchalign::{
    AppState, PreparedWorkers, create_test_app_with_prepared_workers, prepare_workers,
};
use tokio::sync::oneshot;

use super::resolve_python;
use batchalign::host_facts::PerProfile;

/// Opaque per-acquire session identifier. Used by the fixture thread to
/// look up the matching [`ActiveSession`] when releasing.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct SessionId(u64);

/// State held on the dedicated fixture runtime for one acquired session.
struct ActiveSession {
    state: Arc<AppState>,
    runtime_root: tempfile::TempDir,
    server_task: tokio::task::JoinHandle<()>,
}

/// Snapshot of session metadata returned to the test thread.
#[derive(Clone)]
struct SessionSnapshot {
    base_url: String,
    state_dir: PathBuf,
}

/// Backend prepared once on the fixture runtime and shared across every
/// session. The fields mirror what [`create_app_with_prepared_workers`]
/// needs at session-creation time.
struct TestEchoBackend {
    prepared_workers: PreparedWorkers,
    session_config: ServerConfig,
}

impl TestEchoBackend {
    /// Prepare the shared worker pool. Resolves Python lazily so the
    /// fixture thread can return a clean SKIP message instead of
    /// panicking when Python is unavailable.
    async fn initialize() -> Result<Self, String> {
        // Point the host-memory coordinator at a per-process ledger so
        // concurrent test sessions in one binary do not race against each
        // other on the shared default ledger (which the original
        // semaphore-serialized helpers protected by serializing tests).
        // The path lives under the system tempdir and is overwritten at
        // every fixture init; that's fine because each integration-test
        // binary is a single process with a fresh fixture per run.
        isolate_host_memory_ledger();

        let python_path = resolve_python()
            .ok_or_else(|| "Python 3 with batchalign is not available".to_string())?;
        let session_config = test_echo_server_config();
        let prepared_workers =
            prepare_workers(&session_config, test_echo_pool_config(&python_path))
                .await
                .map_err(|error| format!("could not prepare test-echo workers: {error}"))?;
        Ok(Self {
            prepared_workers,
            session_config,
        })
    }
}

/// Re-export of the library's per-process ledger isolator so test
/// helpers in this module and callers of this fixture get one canonical
/// entry point. See `batchalign::host_memory::isolate_host_memory_ledger_for_test`.
pub use batchalign::host_memory::isolate_host_memory_ledger_for_test as isolate_host_memory_ledger;

/// Three-state init slot. Matches the live fixture pattern so a one-time
/// initialization failure (no Python in env, etc.) is cached and every
/// subsequent caller sees the same SKIP message.
enum BackendState {
    Uninitialized,
    Ready(Box<TestEchoBackend>),
    Unavailable(String),
}

/// Commands sent from test threads into the fixture thread.
enum FixtureCommand {
    Acquire {
        /// Optional override of [`ServerConfig`] for this session. When
        /// `None`, the fixture's canned test-echo config is used.
        config_override: Option<ServerConfig>,
        reply: oneshot::Sender<Result<(SessionId, SessionSnapshot), String>>,
    },
    Release {
        id: SessionId,
        reply: oneshot::Sender<()>,
    },
}

/// Test-side handle to the fixture thread.
struct FixtureBridge {
    commands: mpsc::Sender<FixtureCommand>,
    /// Count of successful `prepare_workers` invocations. Exposed via
    /// [`times_prepared`] so smoke tests can assert the warm pool is
    /// shared across every acquire.
    times_prepared: AtomicUsize,
}

static FIXTURE: LazyLock<Arc<FixtureBridge>> = LazyLock::new(start_fixture_thread);

/// Number of times the shared backend has invoked `prepare_workers`.
/// Smoke tests assert this stays at 1 across multiple acquires.
pub fn times_prepared() -> usize {
    FIXTURE.times_prepared.load(Ordering::SeqCst)
}

/// Public handle returned to tests. Holds an HTTP client and the
/// metadata needed to drive the isolated server. Drops trigger an
/// async release of the underlying [`ActiveSession`].
pub struct TestServerSession {
    base_url: String,
    state_dir: PathBuf,
    client: reqwest::Client,
    bridge: Arc<FixtureBridge>,
    /// `Some(id)` while the session owns its [`ActiveSession`] in the
    /// fixture thread; `None` after `close()` or after Drop fires (the
    /// flag suppresses double-release).
    release: Option<SessionId>,
}

impl TestServerSession {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Tear down the session deterministically. Prefer this over the
    /// Drop path inside async tests so axum has a chance to flush.
    pub async fn close(mut self) {
        let Some(id) = self.release.take() else {
            return;
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .bridge
            .commands
            .send(FixtureCommand::Release {
                id,
                reply: reply_tx,
            })
            .is_err()
        {
            return;
        }
        let _ = reply_rx.await;
    }
}

impl Drop for TestServerSession {
    fn drop(&mut self) {
        let Some(id) = self.release.take() else {
            return;
        };
        let bridge = self.bridge.clone();
        thread::spawn(move || release_session_sync(&bridge, id));
    }
}

/// Acquire one isolated test-echo server session over the shared warm
/// worker pool. Returns `None` and prints `SKIP:` to stderr when the
/// shared backend cannot be prepared (typically: no Python in env).
pub async fn acquire_test_server_session() -> Option<TestServerSession> {
    acquire_session_inner(None).await
}

/// Acquire one isolated session with a custom [`ServerConfig`]. The
/// shared warm worker pool is still reused; only the per-session
/// server config (e.g. `max_workers_per_job`, `media_roots`) differs.
pub async fn acquire_test_server_session_with_config(
    config: ServerConfig,
) -> Option<TestServerSession> {
    acquire_session_inner(Some(config)).await
}

async fn acquire_session_inner(config_override: Option<ServerConfig>) -> Option<TestServerSession> {
    let bridge = FIXTURE.clone();
    let (reply_tx, reply_rx) = oneshot::channel();
    if bridge
        .commands
        .send(FixtureCommand::Acquire {
            config_override,
            reply: reply_tx,
        })
        .is_err()
    {
        eprintln!("SKIP: test-echo fixture command channel closed");
        return None;
    }
    let outcome = match reply_rx.await {
        Ok(outcome) => outcome,
        Err(_) => {
            eprintln!("SKIP: test-echo fixture dropped acquire reply");
            return None;
        }
    };
    let (id, snapshot) = match outcome {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("SKIP: {message}");
            return None;
        }
    };

    Some(TestServerSession {
        base_url: snapshot.base_url,
        state_dir: snapshot.state_dir,
        client: reqwest::Client::new(),
        bridge,
        release: Some(id),
    })
}

/// Drop-time release path: runs from a plain `std::thread` so it must
/// block on the oneshot reply. A throwaway current-thread runtime is
/// the simplest blocking adapter for `oneshot::Receiver::await`.
fn release_session_sync(bridge: &Arc<FixtureBridge>, id: SessionId) {
    let (reply_tx, reply_rx) = oneshot::channel();
    if bridge
        .commands
        .send(FixtureCommand::Release {
            id,
            reply: reply_tx,
        })
        .is_err()
    {
        return;
    }
    if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        let _ = rt.block_on(reply_rx);
    }
}

fn start_fixture_thread() -> Arc<FixtureBridge> {
    let (commands, receiver) = mpsc::channel();
    let bridge = Arc::new(FixtureBridge {
        commands,
        times_prepared: AtomicUsize::new(0),
    });

    let thread_bridge = bridge.clone();
    thread::Builder::new()
        .name("batchalign-test-echo-fixture".into())
        .spawn(move || run_fixture_thread(receiver, thread_bridge))
        .expect("test-echo fixture thread should spawn");

    bridge
}

/// Run loop for the dedicated fixture runtime thread. Owns all shared
/// state — prepared backend and active sessions — and processes one
/// command at a time. Concurrent acquires from many tests are still
/// possible because each command is short (binding a port, spawning
/// axum) and per-session work happens on the runtime's task pool.
fn run_fixture_thread(receiver: mpsc::Receiver<FixtureCommand>, bridge: Arc<FixtureBridge>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("test-echo fixture runtime should build");
    let mut backend_state = BackendState::Uninitialized;
    let mut active_sessions: HashMap<SessionId, ActiveSession> = HashMap::new();
    let mut next_session_id: u64 = 0;

    while let Ok(command) = receiver.recv() {
        match command {
            FixtureCommand::Acquire {
                config_override,
                reply,
            } => {
                let backend = match ensure_backend(&runtime, &mut backend_state, &bridge) {
                    Ok(backend) => backend,
                    Err(message) => {
                        let _ = reply.send(Err(message));
                        continue;
                    }
                };

                let id = SessionId(next_session_id);
                next_session_id += 1;
                match runtime.block_on(start_session(backend, config_override)) {
                    Ok((session, snapshot)) => {
                        active_sessions.insert(id, session);
                        let _ = reply.send(Ok((id, snapshot)));
                    }
                    Err(message) => {
                        let _ = reply.send(Err(message));
                    }
                }
            }
            FixtureCommand::Release { id, reply } => {
                if let Some(session) = active_sessions.remove(&id) {
                    runtime.block_on(cleanup_session(session));
                }
                let _ = reply.send(());
            }
        }
    }

    for (_, session) in active_sessions.drain() {
        runtime.block_on(cleanup_session(session));
    }
}

/// Initialize the backend on first use; cache success and failure.
fn ensure_backend<'a>(
    runtime: &tokio::runtime::Runtime,
    state: &'a mut BackendState,
    bridge: &Arc<FixtureBridge>,
) -> Result<&'a TestEchoBackend, String> {
    if matches!(state, BackendState::Uninitialized) {
        *state = match runtime.block_on(TestEchoBackend::initialize()) {
            Ok(backend) => {
                bridge.times_prepared.fetch_add(1, Ordering::SeqCst);
                BackendState::Ready(Box::new(backend))
            }
            Err(message) => BackendState::Unavailable(message),
        };
    }

    match state {
        BackendState::Ready(backend) => Ok(backend),
        BackendState::Unavailable(message) => Err(message.clone()),
        BackendState::Uninitialized => unreachable!("backend should be initialized before use"),
    }
}

/// Build one isolated app + server backed by the shared prepared workers.
async fn start_session(
    backend: &TestEchoBackend,
    config_override: Option<ServerConfig>,
) -> Result<(ActiveSession, SessionSnapshot), String> {
    let runtime_root = tempfile::TempDir::new()
        .map_err(|error| format!("could not create session tempdir: {error}"))?;
    let layout = RuntimeLayout::from_state_dir(runtime_root.path().to_path_buf());
    let cache_dir = runtime_root.path().join("cache");
    let session_config = config_override.unwrap_or_else(|| backend.session_config.clone());
    let (router, state) = create_test_app_with_prepared_workers(
        session_config,
        layout,
        None,
        None,
        Some(cache_dir),
        Some("test-echo-fixture-hash".into()),
        backend.prepared_workers.clone(),
    )
    .await
    .map_err(|error| format!("could not create test-echo app: {error}"))?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| format!("could not bind test-echo listener: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("could not read test-echo local_addr: {error}"))?
        .port();
    let base_url = format!("http://127.0.0.1:{port}");

    let server_task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .ok();
    });

    // 50 ms matches `cli_common::start_test_server`'s historical settle —
    // axum::serve's accept loop only starts when the spawned task gets
    // polled, and a too-fast first request races that scheduling.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let snapshot = SessionSnapshot {
        base_url,
        state_dir: runtime_root.path().to_path_buf(),
    };
    let session = ActiveSession {
        state,
        runtime_root,
        server_task,
    };
    Ok((session, snapshot))
}

async fn cleanup_session(session: ActiveSession) {
    let ActiveSession {
        state,
        runtime_root,
        server_task,
    } = session;
    super::cleanup_active_session(state, runtime_root, server_task, "test-echo fixture").await;
}

fn test_echo_server_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".into(),
        port: 0,
        job_ttl_days: 7,
        memory_gate_mb: Some(MemoryMb(0)),
        ..Default::default()
    }
}

/// Pool config tuned for test-echo: long idle/health timeouts so the
/// shared workers persist across the entire test-binary run; a
/// generous `max_workers_per_key` so concurrent sessions can dispatch
/// in parallel without queueing on a single worker.
fn test_echo_pool_config(python_path: &str) -> PoolConfig {
    PoolConfig {
        python_path: python_path.into(),
        test_echo: true,
        health_check_interval_s: 600,
        ready_timeout_s: 30,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    }
}
