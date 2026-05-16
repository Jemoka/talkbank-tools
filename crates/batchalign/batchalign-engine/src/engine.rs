//! `BatchalignEngine` (spec2.md §11): the per-Pipeline dispatcher.
//!
//! Routes a `TaskInput` to the right backend via an mpsc channel, awaits
//! a oneshot reply, and exposes the route table so `Pipeline` construction
//! can verify backend presence per task.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use batchalign_core::base::Dispatcher;
use batchalign_core::{BAError, BAResult, Backend, Task, TaskInput, TaskOutput};
use tokio::sync::{mpsc, oneshot};

use crate::backend_impl::BackendImpl;
use crate::batcher::{batcher_loop, BatchItem};
use crate::cache::Cache;

/// Tunables for the engine. `max_concurrent_values` is the Pipeline-level
/// semaphore size — used in `pipeline.rs`, kept here so users can tweak it
/// when (and if) the API exposes it.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub max_concurrent_values: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_values: 8,
        }
    }
}

/// Routing table: `Task` → sender into the backend's batcher channel.
///
/// Mutated only at register-time and shutdown-time. Wrapped in `Arc` so the
/// engine can be shared across the Pipeline + per-value futures.
struct RouteTable {
    by_task: HashMap<Task, mpsc::UnboundedSender<BatchItem>>,
}

/// The engine.
///
/// Construction takes the cache; `register` adds backends. `dispatch`
/// translates a `TaskInput` into a routed oneshot await. `shutdown` drops
/// all senders so the batcher loops drain and exit cleanly.
pub struct BatchalignEngine {
    /// Routes are mutated only at register/shutdown time and read by every
    /// dispatch. `RwLock` gives us "many readers, infrequent writers". The
    /// inner HashMap holds `Sender` clones; cheap to copy out before await.
    routes: RwLock<HashMap<Task, mpsc::UnboundedSender<BatchItem>>>,
    cache: Arc<Cache>,
    config: EngineConfig,
}

impl BatchalignEngine {
    pub fn new(cache: Arc<Cache>) -> Self {
        Self::with_config(cache, EngineConfig::default())
    }

    pub fn with_config(cache: Arc<Cache>, config: EngineConfig) -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            cache,
            config,
        }
    }

    /// Registers a backend by:
    /// 1. Reading its name/tasks/policy.
    /// 2. Creating one mpsc channel.
    /// 3. Spawning `batcher_loop` on `handle`.
    /// 4. Inserting per-task route entries pointing at the new channel.
    ///
    /// Errors if a task already has a registered backend (no contention).
    pub fn register(
        &self,
        backend: BackendImpl,
        handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        let policy = backend.batch_policy();
        let name = backend.name().to_string();
        let tasks: Vec<Task> = backend.tasks().to_vec();
        let backend = Arc::new(backend);
        let (tx, rx) = mpsc::unbounded_channel::<BatchItem>();

        handle.spawn(batcher_loop(backend.clone(), policy, self.cache.clone(), rx));

        let mut routes = self
            .routes
            .write()
            .map_err(|_| anyhow!("engine routes lock poisoned"))?;
        for task in tasks {
            if routes.insert(task, tx.clone()).is_some() {
                bail!("two backends declared for {task:?} (second: {name})");
            }
        }
        Ok(())
    }

    /// True if any registered backend declared `task`.
    pub fn serves(&self, task: Task) -> bool {
        match self.routes.read() {
            Ok(g) => g.contains_key(&task),
            Err(_) => false,
        }
    }

    /// Dispatches one input. Awaits the per-call oneshot reply.
    ///
    /// Errors when:
    ///   * no backend handles the input's task,
    ///   * the batcher channel is gone (engine shut down),
    ///   * the batcher dropped the reply mid-flight.
    pub async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
        let task = input.task();
        // Clone the sender out so we don't hold the lock across `.await`.
        let tx = {
            let routes = self
                .routes
                .read()
                .map_err(|_| BAError::Internal("engine routes lock poisoned".into()))?;
            routes
                .get(&task)
                .cloned()
                .ok_or_else(|| BAError::Worker(format!("no backend registered for {task:?}")))?
        };
        let (reply, rx) = oneshot::channel();
        tx.send(BatchItem { input, reply }).map_err(|_| {
            BAError::Worker(format!(
                "engine batcher for {task:?} is gone (engine shut down?)"
            ))
        })?;
        rx.await
            .map_err(|_| BAError::Worker(format!("engine dropped reply for {task:?} mid-call")))?
    }

    /// Drops all per-task senders. Each batcher loop sees `recv() -> None`
    /// and exits. Pending dispatches see `send()` fail and surface
    /// poison-pill errors back through `run_one`.
    pub fn shutdown(&self) {
        if let Ok(mut routes) = self.routes.write() {
            routes.clear();
        }
    }

    /// Test/inspection helper.
    pub fn config(&self) -> EngineConfig {
        self.config
    }
}

/// Implements core's `Dispatcher` so runners written against `&dyn Dispatcher`
/// can drive this engine without depending on the engine crate directly.
#[async_trait]
impl Dispatcher for BatchalignEngine {
    async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
        BatchalignEngine::dispatch(self, input).await
    }
}
