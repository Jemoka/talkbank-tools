//! `BatchalignEngine` (spec2.md §11): the per-Pipeline dispatcher.
//!
//! Routes a `TaskInput` to the right backend via an mpsc channel, awaits
//! a oneshot reply, and exposes the route table so `Pipeline` construction
//! can verify backend presence per task.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use batchalign_core::base::Dispatcher;
use batchalign_core::{
    BAError, BAResult, Backend, BackendProgress, NullBackendProgress, Task, TaskInput, TaskOutput,
};
use tokio::sync::{mpsc, oneshot};

use crate::backend_impl::BackendImpl;
use crate::batcher::{BatchItem, batcher_loop};
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

fn route_queue_capacity(config: EngineConfig) -> usize {
    config.max_concurrent_values.max(1)
}

/// Routing table: `Task` → sender into the backend's batcher channel.
///
/// Mutated only at register-time and shutdown-time. Wrapped in `Arc` so the
/// engine can be shared across the Pipeline + per-value futures.
struct RouteTable {
    by_task: HashMap<Task, mpsc::Sender<BatchItem>>,
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
    routes: RwLock<HashMap<Task, mpsc::Sender<BatchItem>>>,
    /// Backend display name for each registered task. Read at runtime by
    /// runners that want to stamp provenance into generated artifacts
    /// (e.g. ASR's `@Comment` header). Populated alongside `routes`.
    backend_names: RwLock<HashMap<Task, String>>,
    cache: Arc<Cache>,
    config: EngineConfig,
    /// Cooperative cancellation flag (Landing 2 #9). When set, `dispatch`
    /// short-circuits with `BAError::Cancelled` before issuing the batcher
    /// send. Backend `call()` implementations are expected to be
    /// short-running enough that polling between dispatches is sufficient;
    /// long-running backends are responsible for honoring cancellation
    /// internally (e.g. checking the same flag via `cancellation()`).
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl BatchalignEngine {
    pub fn new(cache: Arc<Cache>) -> Self {
        Self::with_config(cache, EngineConfig::default())
    }

    pub fn with_config(cache: Arc<Cache>, config: EngineConfig) -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            backend_names: RwLock::new(HashMap::new()),
            cache,
            config,
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Mark the engine as cancelled. Subsequent `dispatch` calls return
    /// `BAError::Cancelled` immediately; in-flight calls are not
    /// interrupted (cooperative). Idempotent.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Return a clonable cancellation handle. Long-running backends can
    /// poll it; the engine itself checks it at dispatch entry.
    pub fn cancellation(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.cancelled.clone()
    }

    /// Whether `cancel()` has been called.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Registers a backend by:
    /// 1. Reading its name/tasks/policy.
    /// 2. Creating one mpsc channel.
    /// 3. Spawning `batcher_loop` on `handle`.
    /// 4. Inserting per-task route entries pointing at the new channel.
    ///
    /// Errors if a task already has a registered backend (no contention).
    pub fn register(&self, backend: BackendImpl, handle: &tokio::runtime::Handle) -> Result<()> {
        let policy = backend.batch_policy();
        let name = backend.name().to_string();
        let tasks: Vec<Task> = backend.tasks().to_vec();
        let backend = Arc::new(backend);
        // A dispatch item can own an entire decoded PCM file. Bound the
        // per-backend queue to the same admission budget as the pipeline so
        // callers outside `Pipeline::run` cannot accumulate an unbounded
        // resident set behind one slow model. `send().await` below supplies
        // backpressure, and dropping routes during shutdown wakes waiters.
        let queue_capacity = route_queue_capacity(self.config);
        let (tx, rx) = mpsc::channel::<BatchItem>(queue_capacity);

        handle.spawn(batcher_loop(
            backend.clone(),
            policy,
            self.cache.clone(),
            rx,
        ));

        let mut routes = self
            .routes
            .write()
            .map_err(|_| anyhow!("engine routes lock poisoned"))?;
        let mut names = self
            .backend_names
            .write()
            .map_err(|_| anyhow!("engine backend_names lock poisoned"))?;
        for task in tasks {
            if routes.insert(task, tx.clone()).is_some() {
                bail!("two backends declared for {task:?} (second: {name})");
            }
            names.insert(task, name.clone());
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
    #[tracing::instrument(skip(self, input), fields(task = ?input.task()))]
    pub async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
        // Delegate to the progress-threading entry point with a
        // no-op progress channel — keeps the routing logic in one place.
        self.dispatch_inner(
            input,
            Arc::new(NullBackendProgress) as Arc<dyn BackendProgress>,
        )
        .await
    }

    /// Like [`dispatch`] but ships a backend-side progress handle with
    /// the request. The handle is delivered to the backend's
    /// `call_with_progress` and may be invoked as the backend processes
    /// the input. See [`BackendProgress`] for the contract.
    pub async fn dispatch_with_progress(
        &self,
        input: TaskInput,
        progress: Arc<dyn BackendProgress>,
    ) -> BAResult<TaskOutput> {
        self.dispatch_inner(input, progress).await
    }

    async fn dispatch_inner(
        &self,
        input: TaskInput,
        progress: Arc<dyn BackendProgress>,
    ) -> BAResult<TaskOutput> {
        if self.is_cancelled() {
            return Err(BAError::Worker("engine cancelled".into()));
        }
        let task = input.task();
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
        tx.send(BatchItem {
            input,
            reply,
            progress,
        })
        .await
        .map_err(|_| {
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

    async fn dispatch_with_progress(
        &self,
        input: TaskInput,
        progress: Arc<dyn BackendProgress>,
    ) -> BAResult<TaskOutput> {
        BatchalignEngine::dispatch_with_progress(self, input, progress).await
    }

    fn engine_name(&self, task: Task) -> Option<String> {
        self.backend_names.read().ok()?.get(&task).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_route_capacity_matches_memory_admission_budget() {
        assert_eq!(
            route_queue_capacity(EngineConfig {
                max_concurrent_values: 3,
            }),
            3
        );
        assert_eq!(
            route_queue_capacity(EngineConfig {
                max_concurrent_values: 0,
            }),
            1,
            "a zero-sized configuration must still admit one item"
        );
    }
}
