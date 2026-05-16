//! The cache (spec2.md §13).
//!
//! Spec called for rusqlite. The monorepo's libsqlite3-sys story is broken
//! (sqlx 0.8 in talkbank-transform pins a major different from rusqlite 0.34,
//! plus the macOS host SDK can't build the bundled sqlite3.c). We use `redb`,
//! a pure-Rust embedded ACID KV store. Bytes-in, bytes-out — the perfect
//! shape for blake3(input) → serde_json(output) entries.
//!
//! Key layout: blake3(format!("{task:?}|{backend_name}|") || serde_json(input))
//! Value layout: serde_json(TaskOutput) as UTF-8 bytes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use batchalign_core::{Task, TaskInput, TaskOutput};
use pyo3::prelude::*;
use redb::{Database, ReadableTable, TableDefinition};

/// Canonical KV table inside the redb database.
const ENTRIES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("entries");

/// Where the cache lives on disk and how it should be consulted.
///
/// Spec calls this `CacheSpec`; the `#[pyclass]` shape makes it constructible
/// from Python as `batchalign.CacheSpec(path=..., policy=...)`.
#[pyclass]
#[derive(Clone, Debug)]
pub struct CacheSpec {
    #[pyo3(get, set)]
    pub path: PathBuf,
    #[pyo3(get, set)]
    pub policy: CachePolicy,
}

#[pymethods]
impl CacheSpec {
    #[new]
    #[pyo3(signature = (path=None, policy=CachePolicy::Use))]
    fn py_new(path: Option<PathBuf>, policy: CachePolicy) -> Self {
        Self {
            path: path.unwrap_or_else(default_cache_path),
            policy,
        }
    }

    #[staticmethod]
    fn bypass() -> Self {
        Self {
            path: default_cache_path(),
            policy: CachePolicy::Bypass,
        }
    }

    #[staticmethod]
    fn refresh() -> Self {
        Self {
            path: default_cache_path(),
            policy: CachePolicy::Refresh,
        }
    }
}

impl Default for CacheSpec {
    fn default() -> Self {
        Self {
            path: default_cache_path(),
            policy: CachePolicy::Use,
        }
    }
}

/// Cache-consultation policy. Three variants because there are exactly three
/// behaviors users care about: normal, A/B-compare-with-fresh, refresh-stale.
#[pyclass(eq, eq_int)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CachePolicy {
    /// Read + write. Default.
    Use,
    /// Never read, never write. Useful for benchmarking, A/B comparisons.
    Bypass,
    /// Never read, always write. Force recompute + repopulate, e.g. after
    /// the underlying provider was updated without bumping its `name()`.
    Refresh,
}

/// Default cache location: `${CACHE}/batchalign/batchaligncache.redb`.
pub fn default_cache_path() -> PathBuf {
    // dirs::cache_dir is well-defined on macOS / Linux / Windows. On unusual
    // hosts (e.g. minimal Docker images with no $HOME) it returns None; we
    // fall back to a project-local file so we don't panic in long-lived
    // logic (spec rule §0: no unwrap/expect on user-facing paths).
    let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("batchalign").join("batchaligncache.redb")
}

/// `nuke_cache()` — top-level pyfunction, deletes the default cache file.
///
/// Out-of-band reset; doesn't require constructing a Pipeline.
#[pyfunction]
pub fn nuke_cache() -> PyResult<()> {
    let path = default_cache_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("nuke_cache: {e}")))?;
    }
    Ok(())
}

/// Opens a redb database at the requested path and exposes typed get/put.
///
/// Cloning is via `Arc<Cache>` at the call site; `Database` itself isn't
/// `Clone` but `Cache` lives behind `Arc` in the engine.
pub struct Cache {
    db: Database,
    policy: CachePolicy,
}

impl Cache {
    /// Opens (or creates) the cache file at `path` under `policy`.
    ///
    /// Creates parent directories as needed. Empty tables are created lazily
    /// inside the first write transaction.
    pub fn open(path: &Path, policy: CachePolicy) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cache: create parent {}", parent.display()))?;
        }
        let db = Database::create(path)
            .with_context(|| format!("cache: open redb at {}", path.display()))?;
        // Eagerly materialize the table so read txns don't fail on first use.
        {
            let write = db.begin_write()?;
            {
                let _table = write.open_table(ENTRIES)?;
            }
            write.commit()?;
        }
        Ok(Self { db, policy })
    }

    /// Builds the cache key from (task, backend_name, input).
    ///
    /// Format: blake3(`"{task:?}|{backend_name}|"` || serde_json(input)).
    fn key(task: Task, backend_name: &str, input: &TaskInput) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(format!("{task:?}|{backend_name}|").as_bytes());
        // serde_json on a serde-derive enum is infallible for normal data,
        // but `?`-bubble in case of unexpected non-serializable payloads.
        if let Ok(canon) = serde_json::to_vec(input) {
            hasher.update(&canon);
        }
        hasher.finalize().as_bytes().to_vec()
    }

    /// Looks up an entry. Returns None on miss or under Bypass/Refresh.
    pub fn get(&self, task: Task, backend_name: &str, input: &TaskInput) -> Option<TaskOutput> {
        if matches!(self.policy, CachePolicy::Bypass | CachePolicy::Refresh) {
            return None;
        }
        let k = Self::key(task, backend_name, input);
        let read = self.db.begin_read().ok()?;
        let table = read.open_table(ENTRIES).ok()?;
        let access = table.get(k.as_slice()).ok()??;
        let bytes = access.value().to_vec();
        serde_json::from_slice::<TaskOutput>(&bytes).ok()
    }

    /// Inserts an entry. No-op under Bypass.
    pub fn put(&self, task: Task, backend_name: &str, input: &TaskInput, output: &TaskOutput) {
        if matches!(self.policy, CachePolicy::Bypass) {
            return;
        }
        let k = Self::key(task, backend_name, input);
        let v = match serde_json::to_vec(output) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(target: "batchalign::cache", "put: serialize failed: {err}");
                return;
            }
        };
        let write = match self.db.begin_write() {
            Ok(w) => w,
            Err(err) => {
                tracing::warn!(target: "batchalign::cache", "put: begin_write failed: {err}");
                return;
            }
        };
        {
            let mut table = match write.open_table(ENTRIES) {
                Ok(t) => t,
                Err(err) => {
                    tracing::warn!(target: "batchalign::cache", "put: open_table failed: {err}");
                    return;
                }
            };
            if let Err(err) = table.insert(k.as_slice(), v.as_slice()) {
                tracing::warn!(target: "batchalign::cache", "put: insert failed: {err}");
                return;
            }
        }
        if let Err(err) = write.commit() {
            tracing::warn!(target: "batchalign::cache", "put: commit failed: {err}");
        }
    }

    /// Test/inspection helper: the active policy.
    pub fn policy(&self) -> CachePolicy {
        self.policy
    }
}

/// Convenience: open from a `CacheSpec`, returning `Arc<Cache>`.
pub fn open_from_spec(spec: &CacheSpec) -> Result<Arc<Cache>> {
    Ok(Arc::new(Cache::open(&spec.path, spec.policy)?))
}
