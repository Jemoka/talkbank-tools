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
    BAError, BAValue, Chat, ChatInput, DynTaskRunner, MediaInput, PairedInput, Paired,
    ProgressEvent, ProgressKind, ProgressSink,
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
    engine: Arc<BatchalignEngine>,
    runtime: Arc<tokio::runtime::Runtime>,
    sem: Arc<Semaphore>,
}

#[pymethods]
impl Pipeline {
    /// Construct a Pipeline.
    ///
    /// `tasks` is just a list of `Task` enum values — runners are stateless
    /// and canonical, so there's no per-task config dict to thread through.
    /// Per-pipeline tunables live on the backend constructors (e.g.
    /// `StanzaBackend(retokenize=True)`).
    #[new]
    #[pyo3(signature = (tasks, backends, cache=None))]
    fn py_new(
        _py: Python<'_>,
        tasks: Vec<Task>,
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

        // 3. Order the declared task set by `Task::requires()` (no
        // auto-expansion — see `expand_with_requires`'s doc).
        let declared: Vec<Task> = tasks;
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

        // 6. Capability gate — every task dispatches to a backend, including
        // Compare (which uses the native Rust `CompareBackend` from
        // `batchalign_core::backends::compare`).
        for &task in &order {
            if !engine.serves(task) {
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

/// Coerce a single Python input object into a `BAValue`.
///
/// Accepts (in order): `MediaInput` → `BAValue::Media`, `ChatInput` →
/// `BAValue::Chat` (parsed + validated), `PairedInput` → `BAValue::Paired`
/// (both sides parsed + validated), a filesystem path string → `BAValue::Media`,
/// or any object exposing a `.path` and (optional) `.source_id` attribute.
/// Compare pipelines need `PairedInput`; FA / morphotag / translate / coref
/// can take `ChatInput` to skip ASR.
fn convert_py_input(py: Python<'_>, obj: Py<PyAny>) -> PyResult<BAValue> {
    let bound = obj.bind(py);
    if let Ok(m) = bound.extract::<MediaInput>() {
        return Ok(BAValue::Media(m));
    }
    if let Ok(c) = bound.extract::<ChatInput>() {
        return load_chat_input(&c).map(BAValue::Chat);
    }
    if let Ok(p) = bound.extract::<PairedInput>() {
        let main = load_chat_at(&p.main, &p.source_id)?;
        let gold_sid = SourceId::try_new(
            p.gold
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("gold"),
        )
        .map_err(|e| PyValueError::new_err(format!("invalid gold source_id: {e}")))?;
        let gold = load_chat_at(&p.gold, &gold_sid)?;
        return Ok(BAValue::Paired(Paired::new(main, gold)));
    }
    if let Ok(path_str) = bound.extract::<String>() {
        return media_from_string(&path_str);
    }
    // Duck-typed fallback: anything with `.main` + `.gold` is treated as a
    // pair; anything else falls back to `.path` (media or chat depending on
    // extension).
    if let (Ok(main_attr), Ok(gold_attr)) = (
        bound.getattr("main").and_then(|p| p.extract::<String>()),
        bound.getattr("gold").and_then(|p| p.extract::<String>()),
    ) {
        let main_path = std::path::PathBuf::from(&main_attr);
        let gold_path = std::path::PathBuf::from(&gold_attr);
        let sid_attr = bound
            .getattr("source_id")
            .and_then(|s| s.extract::<String>())
            .unwrap_or_else(|_| {
                main_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("input")
                    .to_owned()
            });
        let sid = SourceId::try_new(&sid_attr)
            .map_err(|e| PyValueError::new_err(format!("invalid source_id {sid_attr}: {e}")))?;
        let main = load_chat_at(&main_path, &sid)?;
        let gold_sid = SourceId::try_new(
            gold_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("gold"),
        )
        .map_err(|e| PyValueError::new_err(format!("invalid gold source_id: {e}")))?;
        let gold = load_chat_at(&gold_path, &gold_sid)?;
        return Ok(BAValue::Paired(Paired::new(main, gold)));
    }
    let path_attr = bound
        .getattr("path")
        .and_then(|p| p.extract::<String>())
        .map_err(|e| {
            PyValueError::new_err(format!(
                "input is not MediaInput / ChatInput / PairedInput / path / has no .path: {e}"
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
    let path = std::path::PathBuf::from(&path_attr);
    // Route by extension: `.cha`/`.chat` → BAValue::Chat (parsed), else Media.
    if matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("cha") | Some("chat")
    ) {
        return load_chat_at(&path, &sid).map(BAValue::Chat);
    }
    Ok(BAValue::Media(MediaInput::new(sid, path)))
}

fn media_from_string(path_str: &str) -> PyResult<BAValue> {
    let path = std::path::PathBuf::from(path_str);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| PyValueError::new_err(format!("bad path stem: {path_str}")))?;
    let sid = SourceId::try_new(stem)
        .map_err(|e| PyValueError::new_err(format!("invalid source_id from {path_str}: {e}")))?;
    if matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("cha") | Some("chat")
    ) {
        return load_chat_at(&path, &sid).map(BAValue::Chat);
    }
    Ok(BAValue::Media(MediaInput::new(sid, path)))
}

fn load_chat_input(c: &ChatInput) -> PyResult<Chat> {
    let chat = load_chat_at(&c.path, &c.source_id)?;
    // Attach a sibling media file so audio tasks (FA/align) can decode. CHAT's
    // `@Media:` names the base (e.g. `clip` → `clip.wav`); we look for the
    // `.cha` stem with a known audio extension in the same directory. Harmless
    // when none exists (text-only tasks ignore media).
    Ok(attach_sibling_media(chat, &c.path))
}

/// Audio extensions FA/align can decode, in resolution-preference order.
const MEDIA_EXTS: &[&str] = &["wav", "mp3", "m4a", "flac", "ogg", "mp4"];

/// If an audio file with the `.cha`'s stem sits beside it, attach it as media.
fn attach_sibling_media(chat: Chat, cha_path: &std::path::Path) -> Chat {
    let Some(stem) = cha_path.file_stem() else {
        return chat;
    };
    let dir = cha_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    for ext in MEDIA_EXTS {
        let candidate = dir.join(stem).with_extension(ext);
        if candidate.is_file() {
            let sid = chat.source_id().clone();
            return chat.with_media(MediaInput::new(sid, candidate));
        }
    }
    chat
}

fn load_chat_at(path: &std::path::Path, sid: &SourceId) -> PyResult<Chat> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| PyValueError::new_err(format!("cannot read {}: {e}", path.display())))?;
    Chat::parse(&text, sid.clone())
        .map_err(|e| PyValueError::new_err(format!("parse {}: {e}", path.display())))
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
    let engine: &dyn batchalign_core::base::Dispatcher = inner.engine.as_ref();
    let result = runner.apply(&mut value, engine, sink).await;

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

/// Materialize the task set from what the caller declared. We do NOT
/// auto-expand transitive `Task::requires()` prerequisites — those describe
/// the ordering relationship ("Morphosyntax can't run before UtSeg" *when
/// both are present*), not an implicit "if you ask for Morphosyntax I'll
/// also run ASR + UtSeg". The latter would break legitimate pipelines that
/// start from a `BAValue::Chat` or `BAValue::Paired` (e.g.
/// `[Morphosyntax, Compare]` on a Paired of already-tokenized CHATs needs
/// neither ASR nor UtSeg). Callers who *do* want the full chain just
/// declare each task they want explicitly in `recipes.*`.
fn expand_with_requires(declared: &[Task]) -> HashSet<Task> {
    declared.iter().copied().collect()
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

