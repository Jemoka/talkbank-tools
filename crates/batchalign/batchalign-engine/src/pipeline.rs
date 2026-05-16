//! `Pipeline` (spec2.md §12): the orchestrator exposed to Python.
//!
//! Construction:
//!   1. resolve the cache,
//!   2. spin up a per-Pipeline tokio runtime,
//!   3. expand declared tasks via `Task::requires()` and topo-sort,
//!   4. instantiate canonical runners for each task,
//!   5. register all backends with the engine,
//!   6. verify every backend-requiring task has a backend.
//!
//! Execution: `run` blocks on the runtime; each input is driven through
//! `run_one`, which is infallible (per-value errors become `BAValue::Failed`).
//!
//! Drop calls `cancel`, which clears the engine's route table; the runtime
//! drops shortly after, joining all spawned tasks.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use batchalign_core::{
    BAError, BAValue, DynTaskRunner, MediaInput, ProgressEvent, ProgressKind, ProgressSink,
    SourceId, Task,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::Py;
// `pyo3::PyObject` is the alias `Py<PyAny>` in pyo3 0.28; use it via Py<PyAny>.
use tokio::sync::Semaphore;

use crate::backend_impl::BackendImpl;
use crate::cache::{Cache, CacheSpec};
use crate::engine::BatchalignEngine;

/// The Python-facing pipeline object.
#[pyclass]
pub struct Pipeline {
    inner: Arc<PipelineInner>,
}

struct PipelineInner {
    order: Vec<Task>,
    runners: HashMap<Task, Box<dyn DynTaskRunner>>,
    configs: HashMap<Task, serde_json::Value>,
    engine: Arc<BatchalignEngine>,
    runtime: Arc<tokio::runtime::Runtime>,
    sem: Arc<Semaphore>,
}

#[pymethods]
impl Pipeline {
    /// Construct a Pipeline.
    #[new]
    #[pyo3(signature = (tasks, backends, cache=None))]
    fn py_new(
        py: Python<'_>,
        tasks: Vec<(Task, Py<PyDict>)>,
        backends: Vec<Py<PyAny>>,
        cache: Option<CacheSpec>,
    ) -> PyResult<Self> {
        // 1. Resolve cache.
        let cache_spec = cache.unwrap_or_default();
        let cache = Arc::new(
            Cache::open(&cache_spec.path, cache_spec.policy)
                .map_err(|e| PyRuntimeError::new_err(format!("cache open: {e}")))?,
        );

        // 2. Build tokio runtime (one per Pipeline).
        let worker_threads = num_cpus::get().min(8).max(1);
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(worker_threads)
                .thread_name("ba-rt")
                .enable_all()
                .build()
                .map_err(|e| PyRuntimeError::new_err(format!("tokio init: {e}")))?,
        );

        // 3. Parse task configs and expand the DAG via requires().
        let mut declared: Vec<Task> = Vec::with_capacity(tasks.len());
        let mut configs: HashMap<Task, serde_json::Value> = HashMap::new();
        for (task, dict) in tasks {
            declared.push(task);
            let bound = dict.bind(py);
            let json = pydict_to_json(py, bound)?;
            configs.insert(task, json);
        }
        let task_set = expand_with_requires(&declared);
        let order = topo_sort_stable(&declared, &task_set);

        // 4. Canonical runner per task.
        let mut runners: HashMap<Task, Box<dyn DynTaskRunner>> = HashMap::new();
        for &t in &order {
            runners.insert(t, canonical_runner(t));
        }

        // 5. Engine + backend registration.
        let engine = Arc::new(BatchalignEngine::new(cache));
        for py_backend in backends {
            let wrapped = BackendImpl::from_py(py_backend)?;
            engine
                .register(wrapped, runtime.handle())
                .map_err(|e| PyValueError::new_err(format!("register backend: {e}")))?;
        }

        // 6. Capability gate.
        for &task in &order {
            if task.needs_backend() && !engine.serves(task) {
                return Err(PyValueError::new_err(format!(
                    "no backend registered for required task {task:?}"
                )));
            }
        }

        let sem = Arc::new(Semaphore::new(8));

        Ok(Pipeline {
            inner: Arc::new(PipelineInner {
                order,
                runners,
                configs,
                engine,
                runtime,
                sem,
            }),
        })
    }

    /// Runs the pipeline over `inputs`.
    ///
    /// Each input must be a `MediaInput`, a filesystem path string, or any
    /// duck-typed object with `.path` (+ optional `.source_id`). Returns one
    /// `Outcome` per input.
    #[pyo3(signature = (inputs, callbacks=None))]
    fn run(
        &self,
        py: Python<'_>,
        inputs: Vec<Py<PyAny>>,
        callbacks: Option<Vec<(String, Py<PyAny>)>>,
    ) -> PyResult<Vec<crate::py_outcome::PyOutcome>> {
        let mut sink_pairs: Vec<(SourceId, Py<PyAny>)> = Vec::new();
        if let Some(cbs) = callbacks {
            for (id, cb) in cbs {
                let sid = SourceId::try_new(&id).map_err(|e| {
                    PyValueError::new_err(format!("invalid source_id in callbacks: {e}"))
                })?;
                sink_pairs.push((sid, cb));
            }
        }
        let sink = Arc::new(crate::progress_sink::CallbackSink::from_pairs(sink_pairs)) as Arc<dyn ProgressSink>;
        let bavalues = inputs
            .into_iter()
            .map(|obj| convert_py_input(py, obj))
            .collect::<PyResult<Vec<BAValue>>>()?;
        let inner = self.inner.clone();

        // Release the GIL while the runtime drives async work; runners that
        // need it reacquire via `Python::attach`.
        let outcomes = py.detach(|| {
            inner
                .runtime
                .clone()
                .block_on(async move { run_inner(inner, bavalues, sink).await })
        });
        Ok(outcomes
            .into_iter()
            .map(crate::py_outcome::PyOutcome::from_value)
            .collect())
    }

    /// Best-effort cancel.
    fn cancel(&self) {
        self.inner.engine.shutdown();
    }
}

/// Coerce a single Python input object into a `BAValue::Media`.
fn convert_py_input(py: Python<'_>, obj: Py<PyAny>) -> PyResult<BAValue> {
    let bound = obj.bind(py);
    if let Ok(m) = bound.extract::<MediaInput>() {
        return Ok(BAValue::Media(m));
    }
    if let Ok(path_str) = bound.extract::<String>() {
        let path = std::path::PathBuf::from(&path_str);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PyValueError::new_err(format!("bad path stem: {path_str}")))?;
        let sid = SourceId::try_new(stem)
            .map_err(|e| PyValueError::new_err(format!("invalid source_id from {path_str}: {e}")))?;
        return Ok(BAValue::Media(MediaInput::new(sid, path)));
    }
    let path_attr = bound
        .getattr("path")
        .and_then(|p| p.extract::<String>())
        .map_err(|e| {
            PyValueError::new_err(format!(
                "input is not MediaInput / path / has no .path: {e}"
            ))
        })?;
    let sid_attr = bound
        .getattr("source_id")
        .and_then(|s| s.extract::<String>())
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(&path_attr)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("input")
                .to_owned()
        });
    let sid = SourceId::try_new(&sid_attr)
        .map_err(|e| PyValueError::new_err(format!("invalid source_id {sid_attr}: {e}")))?;
    Ok(BAValue::Media(MediaInput::new(
        sid,
        std::path::PathBuf::from(path_attr),
    )))
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.inner.engine.shutdown();
    }
}

async fn run_inner(
    inner: Arc<PipelineInner>,
    inputs: Vec<BAValue>,
    sink: Arc<dyn ProgressSink>,
) -> Vec<BAValue> {
    let futures = inputs.into_iter().map(|value| {
        let me = inner.clone();
        let sink = sink.clone();
        async move {
            let permit = me.sem.clone().acquire_owned().await;
            let _permit = match permit {
                Ok(p) => p,
                Err(_) => {
                    let sid = value.source_id();
                    return BAValue::Failed {
                        source_id: sid,
                        error: BAError::Internal("semaphore closed".into()),
                        partial: Some(Box::new(value)),
                    };
                }
            };
            run_one(me, value, sink.as_ref()).await
        }
    });
    futures::future::join_all(futures).await
}

async fn run_one(
    inner: Arc<PipelineInner>,
    mut value: BAValue,
    sink: &dyn ProgressSink,
) -> BAValue {
    for &task in &inner.order {
        if value.is_failed() {
            return value;
        }
        value = try_step(inner.clone(), task, value, sink).await;
    }
    let sid = value.source_id();
    sink.emit(ProgressEvent {
        source_id: sid,
        task: None,
        kind: ProgressKind::SourceCompleted,
        completed: 0,
        total: 0,
        label: String::new(),
    });
    value
}

async fn try_step(
    inner: Arc<PipelineInner>,
    task: Task,
    mut value: BAValue,
    sink: &dyn ProgressSink,
) -> BAValue {
    let source_id = value.source_id();
    sink.emit(ProgressEvent {
        source_id: source_id.clone(),
        task: Some(task),
        kind: ProgressKind::StageStarted,
        completed: 0,
        total: 0,
        label: format!("{task:?}"),
    });

    let runner = match inner.runners.get(&task) {
        Some(r) => r,
        None => {
            return BAValue::Failed {
                source_id,
                error: BAError::Internal(format!("no runner for {task:?}")),
                partial: Some(Box::new(value)),
            };
        }
    };
    let cfg = inner
        .configs
        .get(&task)
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let engine: &dyn batchalign_core::base::Dispatcher = inner.engine.as_ref();
    let result = runner.apply(&cfg, &mut value, engine, sink).await;

    match result {
        Ok(()) => {
            sink.emit(ProgressEvent {
                source_id,
                task: Some(task),
                kind: ProgressKind::StageInjected,
                completed: 0,
                total: 0,
                label: format!("{task:?}"),
            });
            value
        }
        Err(e) => {
            sink.emit(ProgressEvent {
                source_id: source_id.clone(),
                task: Some(task),
                kind: ProgressKind::StageFailed,
                completed: 0,
                total: 0,
                label: format!("{e:#}"),
            });
            BAValue::Failed {
                source_id,
                error: e,
                partial: Some(Box::new(value)),
            }
        }
    }
}

/// Walk `Task::requires()` transitively to materialize the full task set.
fn expand_with_requires(declared: &[Task]) -> HashSet<Task> {
    let mut out: HashSet<Task> = HashSet::new();
    let mut stack: Vec<Task> = declared.to_vec();
    while let Some(t) = stack.pop() {
        if out.insert(t) {
            for &req in t.requires() {
                stack.push(req);
            }
        }
    }
    out
}

/// Stable topological sort: Kahn's algorithm with ties broken by
/// "earliest-declared first", then by `Task` discriminant order for
/// synthesized (transitively-required) tasks. Determinism matters for
/// cache identity if we ever fold task order into the key.
fn topo_sort_stable(declared_order: &[Task], task_set: &HashSet<Task>) -> Vec<Task> {
    let mut declaration_rank: BTreeMap<Task, usize> = BTreeMap::new();
    for (i, &t) in declared_order.iter().enumerate() {
        declaration_rank.entry(t).or_insert(i);
    }
    let synthesis_base = declared_order.len();
    let rank_of = |t: Task| -> usize {
        declaration_rank
            .get(&t)
            .copied()
            .unwrap_or(synthesis_base + (t as usize))
    };

    let nodes: Vec<Task> = task_set.iter().copied().collect();
    let mut indegree: HashMap<Task, usize> = nodes.iter().map(|&t| (t, 0_usize)).collect();
    let mut edges: HashMap<Task, Vec<Task>> = nodes.iter().map(|&t| (t, Vec::new())).collect();
    for &t in &nodes {
        for &req in t.requires() {
            if task_set.contains(&req) {
                edges.entry(req).or_default().push(t);
                *indegree.entry(t).or_insert(0) += 1;
            }
        }
    }

    let mut out: Vec<Task> = Vec::with_capacity(nodes.len());
    let mut ready: Vec<Task> = indegree
        .iter()
        .filter_map(|(t, &d)| if d == 0 { Some(*t) } else { None })
        .collect();
    while !ready.is_empty() {
        ready.sort_by_key(|&t| rank_of(t));
        let pick = ready.remove(0);
        out.push(pick);
        if let Some(succs) = edges.remove(&pick) {
            for s in succs {
                if let Some(d) = indegree.get_mut(&s) {
                    *d -= 1;
                    if *d == 0 {
                        ready.push(s);
                    }
                }
            }
        }
    }
    out
}

/// Look up the canonical runner for a task. The core crate owns the bodies;
/// this shim falls back to a typed apply-time error if core hasn't yet
/// shipped a runner for `t`.
fn canonical_runner(t: Task) -> Box<dyn DynTaskRunner> {
    batchalign_core::taskrunners::canonical(t)
}

/// Convert a Python dict to `serde_json::Value` via the stdlib `json` module.
fn pydict_to_json(py: Python<'_>, dict: &Bound<'_, PyDict>) -> PyResult<serde_json::Value> {
    let json_mod = py.import("json")?;
    let s: String = json_mod.getattr("dumps")?.call1((dict,))?.extract()?;
    serde_json::from_str(&s)
        .map_err(|e| PyValueError::new_err(format!("config dict not JSON-serializable: {e}")))
}
