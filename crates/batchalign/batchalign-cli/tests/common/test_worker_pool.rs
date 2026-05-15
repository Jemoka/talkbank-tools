//! Per-test-binary, ConfigKey-keyed Python-worker fixture.
//!
//! Each `cargo nextest run` of the python-workers test-group fork/exec's
//! ~50 `uv run python -m batchalign.worker` processes. At default parallelism
//! the spawn tail under contention exceeded `ready_timeout_s`, producing
//! flaky failures. `.config/nextest.toml` currently caps the group at
//! `max-threads = 4` to make spawn cost an OS-bounded constant rather than
//! a tail distribution. This fixture is the principled fix that makes the
//! cap unnecessary: tests within a binary share workers keyed by the subset
//! of [`WorkerConfig`] the Python child observes at startup.
//!
//! Fields the Python child does NOT observe — `ready_timeout_s`,
//! `audio_task_timeout_s`, `analysis_task_timeout_s`, `runtime.host_memory`,
//! `runtime.memory_tier`, `runtime.server_instance_id`,
//! `runtime.server_process_id` — are deliberately excluded from
//! [`ConfigKey`], so tests that vary only those fields still share a worker.
//! Source of truth for the observed/excluded split is
//! `crate::worker::handle::spawn::build_worker_command`; if a flag or env
//! var is added there, mirror it in [`ConfigKey`].
//!
//! Workers live for the test-binary process lifetime; the `OnceLock` is
//! never dropped, so on process exit the workers become orphans and are
//! reaped on the next batchalign run via the worker registry's PID-file
//! mechanism. Acceptable for `--test-echo` workers (no model state).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use tokio::sync::{Mutex, OnceCell, OwnedMutexGuard};

use batchalign::api::{NumSpeakers, WorkerLanguage};
use batchalign::worker::error::WorkerError;
use batchalign::worker::handle::{WorkerConfig, WorkerHandle};
use batchalign::worker::{InferTask, WorkerBootstrapMode, WorkerProfile};

/// The subset of [`WorkerConfig`] that determines Python-side worker
/// behavior. Two configs with the same `ConfigKey` produce equivalent
/// child processes and may safely share a single worker.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ConfigKey {
    python_path: String,
    profile: WorkerProfile,
    task: Option<InferTask>,
    lang: WorkerLanguage,
    num_speakers: NumSpeakers,
    engine_overrides: String,
    test_echo: bool,
    verbose: u8,
    force_cpu: bool,
    gpu_thread_pool_size: u32,
    bootstrap_mode: WorkerBootstrapMode,
    revai_api_key: Option<String>,
    test_delay_ms: u64,
}

impl ConfigKey {
    fn from_config(config: &WorkerConfig) -> Self {
        Self {
            python_path: config.python_path.clone(),
            profile: config.profile,
            task: config.task,
            lang: config.lang.clone(),
            num_speakers: config.num_speakers,
            engine_overrides: config.engine_overrides.clone(),
            test_echo: config.test_echo,
            verbose: config.verbose,
            force_cpu: config.runtime.force_cpu,
            gpu_thread_pool_size: config.runtime.gpu_thread_pool_size,
            bootstrap_mode: config.runtime.bootstrap_mode,
            revai_api_key: config.runtime.revai_api_key.clone(),
            test_delay_ms: config.test_delay_ms,
        }
    }
}

/// One pool entry: the worker handle is initialized once on first
/// checkout for this key, and serialized for dispatch thereafter.
struct WorkerCell {
    handle: OnceCell<Arc<Mutex<WorkerHandle>>>,
}

/// Per-binary fixture.
pub struct SharedTestWorkerPool {
    cells: Mutex<HashMap<ConfigKey, Arc<WorkerCell>>>,
}

impl SharedTestWorkerPool {
    fn new() -> Self {
        Self {
            cells: Mutex::new(HashMap::new()),
        }
    }

    /// Lease a worker for the given config. If a worker exists for the
    /// matching `ConfigKey`, this awaits exclusive access to it. Otherwise
    /// a new worker is spawned, inserted into the pool, and leased.
    pub async fn checkout(&self, config: &WorkerConfig) -> Result<WorkerLease, WorkerError> {
        let key = ConfigKey::from_config(config);

        // Outer lock is held only long enough to insert an empty
        // `WorkerCell`. Spawning happens under the per-key `OnceCell`,
        // so a slow Python startup for one key never blocks unrelated
        // keys from looking up or initializing their own cells.
        let cell = {
            let mut cells = self.cells.lock().await;
            cells
                .entry(key)
                .or_insert_with(|| {
                    Arc::new(WorkerCell {
                        handle: OnceCell::new(),
                    })
                })
                .clone()
        };

        let handle_arc = cell
            .handle
            .get_or_try_init(|| async {
                let handle = WorkerHandle::spawn(config.clone()).await?;
                Ok::<_, WorkerError>(Arc::new(Mutex::new(handle)))
            })
            .await?
            .clone();

        // Per-worker mutex: serializes dispatch on the one stdin/stdout
        // pipe that talks to a single Python child.
        let guard = handle_arc.lock_owned().await;
        Ok(WorkerLease { guard })
    }
}

/// Exclusive lease on one pooled worker. Drops back into the pool on
/// scope exit. Derefs (mut) to [`WorkerHandle`] so callers use it
/// exactly like a directly-spawned handle.
pub struct WorkerLease {
    guard: OwnedMutexGuard<WorkerHandle>,
}

impl std::ops::Deref for WorkerLease {
    type Target = WorkerHandle;

    fn deref(&self) -> &WorkerHandle {
        &self.guard
    }
}

impl std::ops::DerefMut for WorkerLease {
    fn deref_mut(&mut self) -> &mut WorkerHandle {
        &mut self.guard
    }
}

/// Per-binary singleton accessor. Each integration-test target compiled
/// as its own crate gets its own static area, so this is automatically
/// scoped per binary without thread-locals or external keying.
pub fn shared_test_worker_pool() -> &'static SharedTestWorkerPool {
    static POOL: OnceLock<SharedTestWorkerPool> = OnceLock::new();
    POOL.get_or_init(SharedTestWorkerPool::new)
}
