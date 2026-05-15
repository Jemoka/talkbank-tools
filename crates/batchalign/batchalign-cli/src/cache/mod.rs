//! Utterance-level NLP result cache for the batchalign3 server.
//!
//! This crate provides a persistent cache for expensive NLP inference results
//! (morphosyntax, utterance segmentation, forced alignment, translation) so
//! that re-processing a corpus skips utterances whose results are already
//! known.  Each cache entry is keyed by a content-derived SHA-256 hash and
//! scoped to a task name and engine version, so upgrading a model (e.g.
//! Stanza 1.8 to 1.9) automatically invalidates stale entries.
//!
//! # Python compatibility
//!
//! The SQLite schema, key formulas, and database file path are identical to
//! the Python `CacheManager` in `batchalign/pipelines/cache.py`.  Both Rust
//! and Python processes can read and write the same `cache.db` concurrently
//! thanks to SQLite WAL mode.  This allows the Rust server and the Python
//! processing pipeline to share a single cache during the migration period.
//!
//! # Database location
//!
//! The default database path follows platform conventions via the [`dirs`]
//! crate, matching Python's `platformdirs.user_cache_dir("batchalign3",
//! "batchalign3")`:
//!
//! | Platform | Path |
//! |----------|------|
//! | macOS    | `~/Library/Caches/batchalign3/cache.db` |
//! | Linux    | `~/.cache/batchalign3/cache.db` |
//!
//! A custom directory can be passed to [`UtteranceCache::sqlite`] for test
//! isolation.
//!
//! # Architecture
//!
//! The crate is organized around the [`CacheBackend`] trait, which defines
//! the storage contract (get, put, delete -- both single and batched).
//!
//! The production configuration is a **tiered cache**: a
//! [`TieredCacheBackend`] wrapping [`SqliteBackend`].  The hot layer is a
//! [moka](https://github.com/moka-rs/moka) `future::Cache` (10,000 entries,
//! 24h time-to-idle) that absorbs repeated lookups and reduces SQLite
//! round-trips under concurrent workloads.  SQLite remains the authoritative
//! persistent store; writes go through both layers (write-through).
//!
//! [`UtteranceCache`] is the public entry point.  It wraps a
//! `Box<dyn CacheBackend>` and provides factory methods:
//!
//! - [`UtteranceCache::tiered`] -- open a tiered cache (moka hot + SQLite
//!   cold).  This is the default used in production.
//! - [`UtteranceCache::sqlite`] -- open a plain SQLite cache (no hot layer).
//! - [`UtteranceCache::from_backend`] -- inject a custom backend (e.g. for
//!   testing with an in-memory store).
//!
//! Because `UtteranceCache` itself implements `CacheBackend` (by delegation),
//! callers can use it interchangeably wherever a `&dyn CacheBackend` is
//! expected.
//!
//! # Modules
//!
//! | Module      | Purpose |
//! |-------------|---------|
//! | [`backend`] | [`CacheBackend`] trait definition and [`CacheStats`] type |
//! | `sqlite`    | [`SqliteBackend`] -- WAL-mode SQLite implementation |
//! | `tiered`    | [`TieredCacheBackend`] -- moka hot layer + cold backend |
//!
//! # Examples
//!
//! ```no_run
//! use crate::cache::{UtteranceCache, CacheBackend};
//!
//! # async fn example() -> Result<(), crate::cache::CacheError> {
//! // Open the default tiered cache (moka hot + SQLite cold).
//! let cache = UtteranceCache::tiered(None, None).await?;
//!
//! // Store a morphosyntax result.
//! let key = "a1b2c3d4e5f6..."; // SHA-256 of utterance text + lang
//! let data = serde_json::json!({
//!     "mor": "det|the n|dog v|run-3S",
//!     "gra": "1|2|DET 2|3|SUBJ 3|0|ROOT"
//! });
//! cache.put(key, "morphosyntax", "stanza-1.9.2", "3.1.0", &data).await?;
//!
//! // Retrieve it (only if engine version matches).
//! let hit = cache.get(key, "morphosyntax", "stanza-1.9.2").await?;
//! assert_eq!(hit, Some(data));
//!
//! // A different engine version returns None (cache miss).
//! let miss = cache.get(key, "morphosyntax", "stanza-2.0.0").await?;
//! assert_eq!(miss, None);
//! # Ok(())
//! # }
//! ```
//!
//! Using a temporary directory for test isolation:
//!
//! ```no_run
//! use crate::cache::{UtteranceCache, CacheBackend};
//!
//! # async fn example() -> Result<(), crate::cache::CacheError> {
//! let tmp = tempfile::TempDir::new().unwrap();
//! let cache = UtteranceCache::sqlite(Some(tmp.path().to_path_buf())).await?;
//!
//! cache.put("k1", "utseg", "stanza-1.9", "3.0.0", &serde_json::json!({"seg": [3, 7]}))
//!     .await?;
//!
//! let stats = cache.stats().await?;
//! assert_eq!(stats.total_entries, 1);
//! # Ok(())
//! # }
//! ```

mod backend;
mod noop;
mod sqlite;
mod tiered;

pub use backend::{CacheBackend, CacheStats};
pub use sqlite::SqliteBackend;
pub use tiered::TieredCacheBackend;

use std::path::PathBuf;

/// High-level cache wrapper.
///
/// Wraps a `Box<dyn CacheBackend>` and provides factory methods for
/// the supported backends.
pub struct UtteranceCache {
    backend: Box<dyn CacheBackend>,
}

impl UtteranceCache {
    /// Create a local SQLite-backed cache.
    ///
    /// If `cache_dir` is `None`, uses the platform default:
    /// `~/Library/Caches/batchalign3` on macOS (matching Python's
    /// `platformdirs.user_cache_dir("batchalign3", "batchalign3")`).
    pub async fn sqlite(cache_dir: Option<PathBuf>) -> Result<Self, CacheError> {
        let backend = SqliteBackend::open(cache_dir).await?;
        Ok(Self {
            backend: Box::new(backend),
        })
    }

    /// Create a tiered cache: moka in-memory hot layer + SQLite cold backend.
    ///
    /// - `cache_dir`: SQLite directory (None = platform default).
    /// - `max_hot_entries`: hot-cache capacity (None = 10,000 entries).
    pub async fn tiered(
        cache_dir: Option<PathBuf>,
        max_hot_entries: Option<u64>,
    ) -> Result<Self, CacheError> {
        let cold = SqliteBackend::open(cache_dir).await?;
        let tiered = TieredCacheBackend::new(Box::new(cold), max_hot_entries);
        Ok(Self {
            backend: Box::new(tiered),
        })
    }

    /// Create a no-op cache that always misses.
    ///
    /// Use this for tasks where caching adds overhead without meaningful
    /// benefit (e.g., text NLP tasks where re-inference with warm workers
    /// is faster than SQLite lookups on a large cache). Puts are silently
    /// discarded; gets always return `None`.
    pub fn noop() -> Self {
        Self {
            backend: Box::new(noop::NoopBackend),
        }
    }

    /// Create a cache from an existing backend (for testing or custom backends).
    pub fn from_backend(backend: Box<dyn CacheBackend>) -> Self {
        Self { backend }
    }

    /// Access the underlying backend.
    pub fn backend(&self) -> &dyn CacheBackend {
        &*self.backend
    }
}

// Delegate all CacheBackend methods to the inner backend.
#[async_trait::async_trait]
impl CacheBackend for UtteranceCache {
    async fn get(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
    ) -> Result<Option<serde_json::Value>, CacheError> {
        self.backend.get(key, task, engine_version).await
    }

    async fn get_batch(
        &self,
        keys: &[String],
        task: &str,
        engine_version: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, CacheError> {
        self.backend.get_batch(keys, task, engine_version).await
    }

    async fn put(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
        ba_version: &str,
        data: &serde_json::Value,
    ) -> Result<(), CacheError> {
        self.backend
            .put(key, task, engine_version, ba_version, data)
            .await
    }

    async fn put_batch(
        &self,
        entries: &[(String, serde_json::Value)],
        task: &str,
        engine_version: &str,
        ba_version: &str,
    ) -> Result<(), CacheError> {
        self.backend
            .put_batch(entries, task, engine_version, ba_version)
            .await
    }

    async fn delete_batch(&self, keys: &[String], task: &str) -> Result<usize, CacheError> {
        self.backend.delete_batch(keys, task).await
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        self.backend.stats().await
    }
}

/// Cache errors.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Database migration failed.
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Filesystem I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Platform cache directory could not be determined.
    #[error("Cache directory not found")]
    NoCacheDir,
}
