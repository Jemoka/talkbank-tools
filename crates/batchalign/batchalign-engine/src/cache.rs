//! The cache (spec2.md §13).
//!
//! Backed by LMDB via the `heed` crate. LMDB was chosen over redb because
//! redb is single-writer-per-process: when two binaries from this
//! workspace (CLI + TUI + dashboard) try to share the cache, the second
//! open errors out immediately. LMDB allows multiple processes to read
//! and write the same database concurrently — readers are MVCC-snapshot,
//! and writers serialize via a file lock (`lock.mdb`).
//!
//! ## Pure-Rust posture
//!
//! `heed` ships LMDB as vendored C compiled by `cc-rs` at build time.
//! No system libraries are required at runtime; the binary is statically
//! linked. Build works under both `cargo` and `bazel` + `crate_universe`
//! with no special annotation (verified at migration time).
//!
//! ## Key/value shape
//!
//! Same as the previous redb cache:
//!
//! - Key: `blake3("v2|" || format!("{task:?}|{backend_name}|") || CacheKey::hash(input))`
//! - Value: `serde_json(TaskOutput)` UTF-8 bytes
//!
//! Routing-only fields (`source_id`, `utterance_id`) are excluded from
//! the key via `CacheKey::hash`, so identical content from different
//! files collapses to one entry.
//!
//! ## LMDB single-writer discipline
//!
//! LMDB allows N concurrent reader processes but only ONE writer
//! transaction at a time (across all processes — the lock is on
//! `lock.mdb`, file-level). `write_txn()` blocks the calling thread
//! until any other writer commits. Two consequences for this module:
//!
//! 1. **Keep write txns small.** Each `put` opens a write txn, performs
//!    a single `db.put`, and commits. Serialization (`serde_json::to_vec`)
//!    happens BEFORE acquiring the write txn so no other writer waits on
//!    our CPU work. Do not add expensive operations between
//!    `write_txn()` and `commit()`.
//!
//! 2. **`put` is blocking.** Callers in an async context MUST wrap
//!    `put` in `tokio::task::spawn_blocking` (or equivalent) so the
//!    blocking syscall on the LMDB lock doesn't park a tokio worker.
//!    The batcher does this; see `batcher.rs`.
//!
//! 3. **Don't hold read txns.** A long-lived `RoTxn` pins old pages
//!    against LMDB's free-list reclamation and causes the map to grow
//!    unboundedly. Every `get` here opens a read txn, reads, and drops
//!    it before returning. Do not stash an `RoTxn` anywhere.
//!
//! Cross-process: if another process holds the writer lock, our
//! `write_txn` blocks until that process commits (or dies, in which
//! case the OS releases the file lock and we proceed). No retry or
//! timeout logic in this module — LMDB's blocking is the right
//! primitive for "wait my turn."
//!
//! ## Map size
//!
//! LMDB requires a max memory-map size at `Env::open` time — this is
//! virtual address space, NOT pre-allocated disk. We pick a generous
//! 16 GiB so we never have to deal with `MDB_MAP_FULL` in practice.
//! If the cache ever grows past this, the constant is the single knob
//! to bump and `nuke_cache` is the escape hatch.
//!
//! ## On-disk layout
//!
//! LMDB writes a directory containing `data.mdb` + `lock.mdb`. The
//! default cache location is `${CACHE}/batchalign/cache.lmdb/` — note
//! the trailing component is a directory, not a single file like the
//! old `batchaligncache.redb`. `nuke_cache()` removes the whole
//! directory.
//!
//! ## Known caveat — no backend-code-version stamp
//!
//! The cache keys on `task`, `backend.name()`, and the serialized input.
//! It does NOT include a hash of the backend's *implementation*. Two
//! consequences:
//!
//! 1. If a backend's `name()` is stable across a code change (which it
//!    usually is — backend names are user-visible), edits to the algorithm
//!    silently serve old outputs.
//! 2. Bump the backend's `name` suffix (e.g. `compare:rust:v1` → `:v2`)
//!    every time you change behaviour, or call `nuke_cache()` after a
//!    rebuild.
//!
//! TODO(spec2.md follow-up): add a `code_version: u64` field to
//! `BackendMeta` and include it in the cache key so this discipline isn't
//! manual.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use batchalign_core::{CacheKey, Task, TaskInput, TaskOutput};
use heed::types::Bytes;
use heed::{Database, Env, EnvOpenOptions};
use pyo3::prelude::*;

/// LMDB max-map-size in bytes. Virtual address space, not disk. See
/// module docs.
const CACHE_MAP_SIZE: usize = 16 * 1024 * 1024 * 1024; // 16 GiB

/// Name of the single named database inside the LMDB env. LMDB allows
/// multiple named DBs in one env; we only use one — "entries".
const ENTRIES_DB_NAME: &str = "entries";

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

/// Default cache location: `${CACHE}/batchalign/cache.lmdb/`.
///
/// Note: this is a directory (LMDB creates `data.mdb` + `lock.mdb`
/// inside), not a single file like the old redb path.
pub fn default_cache_path() -> PathBuf {
    // dirs::cache_dir is well-defined on macOS / Linux / Windows. On unusual
    // hosts (e.g. minimal Docker images with no $HOME) it returns None; we
    // fall back to a project-local file so we don't panic in long-lived
    // logic (spec rule §0: no unwrap/expect on user-facing paths).
    let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("batchalign").join("cache.lmdb")
}

/// `nuke_cache()` — top-level pyfunction, deletes the default cache.
///
/// Out-of-band reset; doesn't require constructing a Pipeline. LMDB
/// stores the cache as a directory containing `data.mdb` + `lock.mdb`,
/// so we recursively delete the directory.
#[pyfunction]
pub fn nuke_cache() -> PyResult<()> {
    let path = default_cache_path();
    if path.exists() {
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("nuke_cache: {e}")))?;
        } else {
            // Older single-file (redb) layout — still nuke it.
            std::fs::remove_file(&path)
                .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("nuke_cache: {e}")))?;
        }
    }
    Ok(())
}

/// Process-wide registry of opened LMDB envs, keyed by canonical path.
///
/// LMDB enforces a strict "one Env per (process, path)" rule and
/// errors on the second open. That's a problem because nothing
/// prevents the engine from being constructed twice in one Python
/// session (notebooks, test setups, the `Pipeline` API in general)
/// each pointing at the default cache path. The registry caches the
/// `Env` so the second `Cache::open` reuses the already-opened
/// handle.
///
/// Keyed by canonicalized path so `~/foo` and `/Users/x/foo` collapse
/// to the same entry. Lock contention is irrelevant here — `open` is
/// called O(1) times per process lifetime, not per dispatch.
static ENV_REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Env>>> = OnceLock::new();

fn env_registry() -> &'static Mutex<HashMap<PathBuf, Env>> {
    ENV_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Opens an LMDB environment at the requested path and exposes typed
/// get/put. Multi-process-safe: concurrent CLIs, TUIs, and dashboards
/// can all open the same path without coordination — LMDB serializes
/// writers via its lock file and gives readers MVCC snapshots.
pub struct Cache {
    env: Env,
    db: Database<Bytes, Bytes>,
    policy: CachePolicy,
}

impl Cache {
    /// Opens (or creates) the cache at `path` under `policy`.
    ///
    /// `path` is a directory — LMDB creates `data.mdb` + `lock.mdb`
    /// inside. The directory (and any parents) are created if missing.
    /// The named database is materialized in an initial write txn so
    /// later read txns don't fail with "no such database".
    ///
    /// Within a single process, repeated calls against the same path
    /// share one underlying `Env` via [`ENV_REGISTRY`] — LMDB only
    /// allows one Env per (process, path) and errors on a second
    /// physical open. Re-opens with a different `policy` are allowed;
    /// only the env itself is shared.
    pub fn open(path: &Path, policy: CachePolicy) -> Result<Self> {
        // LMDB opens a directory; create it (and any parents) up front.
        std::fs::create_dir_all(path)
            .with_context(|| format!("cache: create LMDB dir {}", path.display()))?;

        // Canonicalize so logically-equivalent paths collapse to one
        // registry entry. `canonicalize` requires the path to exist —
        // we just created it, so this is fine.
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("cache: canonicalize {}", path.display()))?;

        // Look up or open the env. Holding the mutex across the LMDB
        // open is intentional: it serializes concurrent first-open
        // attempts within this process so we never race two opens
        // against the same path.
        let env = {
            let mut reg = env_registry()
                .lock()
                .map_err(|_| anyhow::anyhow!("cache: env registry mutex poisoned"))?;
            if let Some(existing) = reg.get(&canonical) {
                existing.clone()
            } else {
                // SAFETY: `EnvOpenOptions::open` is unsafe because LMDB
                // maps the file into memory and the caller must promise
                // no other process modifies the file via a non-LMDB
                // path concurrently. We satisfy that by only ever
                // touching the cache through this module — there is
                // no out-of-band writer.
                let env = unsafe {
                    EnvOpenOptions::new()
                        .map_size(CACHE_MAP_SIZE)
                        // One named DB ("entries"). LMDB requires
                        // `max_dbs >= 1` for named-database mode; pad
                        // to 4 to leave room for future indexes /
                        // metadata tables without re-opening.
                        .max_dbs(4)
                        .open(&canonical)
                        .with_context(|| {
                            format!("cache: open LMDB at {}", canonical.display())
                        })?
                };
                reg.insert(canonical.clone(), env.clone());
                env
            }
        };

        // Create-or-open the entries database in an initial write txn.
        // `create_database` is idempotent — if the DB already exists
        // it's just opened. Committing here makes the DB visible to
        // subsequent read txns. Cheap to re-run on a shared env.
        let db = {
            let mut wtxn = env
                .write_txn()
                .context("cache: open initial write txn")?;
            let db: Database<Bytes, Bytes> = env
                .create_database(&mut wtxn, Some(ENTRIES_DB_NAME))
                .context("cache: create/open entries database")?;
            wtxn.commit().context("cache: commit initial txn")?;
            db
        };

        Ok(Self { env, db, policy })
    }

    /// Builds the cache key from (task, backend_name, input).
    ///
    /// Format:
    /// `blake3(v2 || "{task:?}|{backend_name}|" || CacheKey::hash(input))`.
    ///
    /// The `v2` prefix is a schema epoch — bump it whenever the trait
    /// contract changes (e.g. a proto adds a new content-identifying
    /// field). Bumping invalidates the old keyspace cleanly instead of
    /// silently mixing two semantics in the same database.
    ///
    /// Crucially, the input bytes come from `CacheKey::hash`, not raw
    /// `serde_json(input)` — that way routing-only fields like
    /// `source_id` and `utterance_id` do NOT participate, and identical
    /// content from different files / utterance slots collapses to one
    /// cache entry.
    fn key(task: Task, backend_name: &str, input: &TaskInput) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"v2|");
        hasher.update(format!("{task:?}|{backend_name}|").as_bytes());
        input.hash(&mut hasher);
        hasher.finalize().as_bytes().to_vec()
    }

    /// Looks up an entry. Returns None on miss or under Bypass/Refresh.
    pub fn get(&self, task: Task, backend_name: &str, input: &TaskInput) -> Option<TaskOutput> {
        if matches!(self.policy, CachePolicy::Bypass | CachePolicy::Refresh) {
            tracing::debug!(target: "batchalign::cache", ?task, backend = backend_name, policy = ?self.policy, "get: skipped by policy");
            return None;
        }
        let k = Self::key(task, backend_name, input);
        let k_hex: String = k[..8.min(k.len())].iter().map(|b| format!("{b:02x}")).collect();

        let rtxn = match self.env.read_txn() {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: "batchalign::cache", "get: read_txn failed: {e}");
                return None;
            }
        };
        let bytes: Vec<u8> = match self.db.get(&rtxn, k.as_slice()) {
            Ok(Some(slice)) => slice.to_vec(),
            Ok(None) => {
                tracing::info!(target: "batchalign::cache", outcome = "miss", ?task, backend = backend_name, key = %k_hex);
                return None;
            }
            Err(e) => {
                tracing::warn!(target: "batchalign::cache", "get: db.get failed: {e}");
                return None;
            }
        };
        match serde_json::from_slice::<TaskOutput>(&bytes) {
            Ok(o) => {
                tracing::info!(target: "batchalign::cache", outcome = "hit", ?task, backend = backend_name, key = %k_hex);
                Some(o)
            }
            Err(e) => {
                tracing::warn!(target: "batchalign::cache", "get: deserialize failed: {e} (treating as miss)");
                None
            }
        }
    }

    /// Inserts an entry. No-op under Bypass.
    ///
    /// Concurrent writers across processes are serialized by LMDB's
    /// file lock — if another process holds the write txn, `write_txn`
    /// blocks until the lock is released. Within reasonable cache-write
    /// volumes this is invisible.
    pub fn put(&self, task: Task, backend_name: &str, input: &TaskInput, output: &TaskOutput) {
        if matches!(self.policy, CachePolicy::Bypass) {
            return;
        }
        let k = Self::key(task, backend_name, input);
        let k_hex: String = k[..8.min(k.len())].iter().map(|b| format!("{b:02x}")).collect();
        tracing::info!(target: "batchalign::cache", outcome = "put", ?task, backend = backend_name, key = %k_hex);
        let v = match serde_json::to_vec(output) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(target: "batchalign::cache", "put: serialize failed: {err}");
                return;
            }
        };
        let mut wtxn = match self.env.write_txn() {
            Ok(w) => w,
            Err(err) => {
                tracing::warn!(target: "batchalign::cache", "put: write_txn failed: {err}");
                return;
            }
        };
        if let Err(err) = self.db.put(&mut wtxn, k.as_slice(), v.as_slice()) {
            tracing::warn!(target: "batchalign::cache", "put: db.put failed: {err}");
            return;
        }
        if let Err(err) = wtxn.commit() {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Two `Cache` instances opened against the same path can both read
    /// and write without one of them erroring on `open` — this is the
    /// whole reason we moved off redb.
    #[test]
    fn two_cache_handles_share_one_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cache.lmdb");

        let a = Cache::open(&path, CachePolicy::Use).expect("open a");
        let b = Cache::open(&path, CachePolicy::Use).expect("open b");

        // Both handles must observe the same policy / shape.
        assert_eq!(a.policy(), CachePolicy::Use);
        assert_eq!(b.policy(), CachePolicy::Use);
    }

    /// Cross-handle visibility: a `put` on one handle must be visible
    /// to a `get` on another handle opened against the same path.
    /// (LMDB MVCC snapshots; a fresh read txn after the put commit
    /// sees the new value.)
    ///
    /// We can't easily test this without a real `TaskInput`/`TaskOutput`,
    /// so we exercise the raw env+db API directly here — same code path
    /// as `get`/`put` but without the typed wrappers.
    #[test]
    fn cross_handle_visibility() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cache.lmdb");

        let a = Cache::open(&path, CachePolicy::Use).expect("open a");
        let b = Cache::open(&path, CachePolicy::Use).expect("open b");

        // Write through `a`.
        {
            let mut wtxn = a.env.write_txn().unwrap();
            a.db.put(&mut wtxn, b"hello".as_slice(), b"world".as_slice())
                .unwrap();
            wtxn.commit().unwrap();
        }

        // Read through `b`. Must see `a`'s write.
        {
            let rtxn = b.env.read_txn().unwrap();
            let got = b.db.get(&rtxn, b"hello".as_slice()).unwrap();
            assert_eq!(got, Some(b"world".as_slice()));
        }
    }

    /// The whole reason we migrated off redb: two SEPARATE PROCESSES
    /// must be able to open and use the same cache concurrently.
    ///
    /// Strategy: launch two child processes (this same test binary,
    /// re-invoked with a `BATCHALIGN_CACHE_CHILD_*` env hint) that
    /// each open the cache and write/read a known key. Verify both
    /// succeed and see consistent state.
    ///
    /// We use the standard "re-exec self with a magic env var"
    /// approach because Cargo doesn't ship a way to build an
    /// auxiliary binary just for this test.
    #[test]
    fn two_processes_share_one_cache() {
        // Child-process trampoline: when set, do the write/read
        // dance and exit. The parent path branch runs the actual
        // assertions.
        if let Ok(path) = std::env::var("BATCHALIGN_CACHE_CHILD_PATH") {
            let key = std::env::var("BATCHALIGN_CACHE_CHILD_KEY").unwrap();
            let val = std::env::var("BATCHALIGN_CACHE_CHILD_VAL").unwrap();
            let cache =
                Cache::open(std::path::Path::new(&path), CachePolicy::Use).expect("child open");
            let mut wtxn = cache.env.write_txn().expect("child write_txn");
            cache
                .db
                .put(&mut wtxn, key.as_bytes(), val.as_bytes())
                .expect("child put");
            wtxn.commit().expect("child commit");
            // Read back through the SAME handle to confirm the put
            // is durable from this process's POV.
            let rtxn = cache.env.read_txn().expect("child read_txn");
            let got = cache.db.get(&rtxn, key.as_bytes()).expect("child get");
            assert_eq!(got, Some(val.as_bytes()));
            // Exit immediately so the OS releases the LMDB lock file.
            std::process::exit(0);
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cache.lmdb");
        let exe = std::env::current_exe().expect("current_exe");

        // Spawn two children writing different keys. They may overlap
        // in time; LMDB's writer lock serializes them transparently.
        let mut children = Vec::new();
        for (key, val) in [("alpha", "one"), ("beta", "two")] {
            let child = std::process::Command::new(&exe)
                .arg("--exact")
                .arg("cache::tests::two_processes_share_one_cache")
                .arg("--nocapture")
                .env("BATCHALIGN_CACHE_CHILD_PATH", &path)
                .env("BATCHALIGN_CACHE_CHILD_KEY", key)
                .env("BATCHALIGN_CACHE_CHILD_VAL", val)
                .spawn()
                .expect("spawn child");
            children.push(child);
        }
        for mut c in children {
            let status = c.wait().expect("wait child");
            assert!(status.success(), "child exited with {status:?}");
        }

        // After both children exit, open a fresh handle (in this
        // process) and verify both writes landed. This is the
        // multi-process write durability assertion.
        let cache = Cache::open(&path, CachePolicy::Use).expect("parent open");
        let rtxn = cache.env.read_txn().expect("parent read_txn");
        assert_eq!(
            cache.db.get(&rtxn, b"alpha".as_slice()).unwrap(),
            Some(b"one".as_slice()),
            "first child's write must be visible to parent"
        );
        assert_eq!(
            cache.db.get(&rtxn, b"beta".as_slice()).unwrap(),
            Some(b"two".as_slice()),
            "second child's write must be visible to parent"
        );
    }
}
