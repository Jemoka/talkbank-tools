//! Cache backend trait and associated types.
//!
//! This module defines [`CacheBackend`], the storage abstraction that all
//! cache implementations must satisfy, and [`CacheStats`], a diagnostic
//! snapshot of cache contents.
//!
//! # Trait contract
//!
//! [`CacheBackend`] models a key-value store with three dimensions per entry:
//!
//! 1. **`key`** -- a content-derived identifier (typically SHA-256 of the
//!    utterance text, language, and any task-specific parameters).
//! 2. **`task`** -- the NLP task name (e.g. `"morphosyntax"`,
//!    `"utterance_segmentation"`, `"forced_alignment"`, `"translation"`).
//! 3. **`engine_version`** -- the version string of the model that produced
//!    the result (e.g. `"stanza-1.9.2"`).
//!
//! A cache hit requires all three to match: the key must exist, the task
//! must match, and the stored `engine_version` must equal the requested one.
//! This ensures that upgrading a model automatically invalidates stale
//! entries without requiring an explicit cache flush.
//!
//! Values are stored as opaque [`serde_json::Value`] blobs, giving each task
//! freedom to define its own result schema.
//!
//! All methods are async. The trait requires `Send + Sync` so that an
//! `Arc<dyn CacheBackend>` can be shared across async tasks in the server.
//!
//! # Batch operations
//!
//! [`get_batch`](CacheBackend::get_batch) and
//! [`put_batch`](CacheBackend::put_batch) exist for performance: the
//! morphosyntax orchestrator may need to look up hundreds of utterances at
//! once.  The SQLite implementation uses chunked `IN (...)` clauses and
//! single-transaction inserts to minimize round trips.
//!
//! # Implementing a new backend
//!
//! To add a new storage backend (e.g. Postgres):
//!
//! 1. Create a new module (e.g. `postgres.rs`) in this crate.
//! 2. Implement [`CacheBackend`] for your type.
//! 3. Add a factory method on [`UtteranceCache`](crate::UtteranceCache)
//!    (e.g. `UtteranceCache::postgres(url)`).
//! 4. Wire it into `AppState` in `batchalign-server`.
//!
//! The existing [`SqliteBackend`](crate::SqliteBackend) in the sibling
//! `sqlite` module serves as the reference implementation.  The
//! [`TieredCacheBackend`](crate::TieredCacheBackend) in `tiered` shows
//! how to compose backends (moka hot layer wrapping any cold backend).

use std::collections::HashMap;

use crate::cache::CacheError;

/// Statistics about a cache backend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    /// Human-readable location string (e.g. file path or URL).
    pub location: String,
    /// Total size in bytes (if applicable).
    pub size_bytes: u64,
    /// Total number of cached entries.
    pub total_entries: u64,
    /// Entry count by task name.
    pub by_task: HashMap<String, u64>,
    /// Entry count by "task engine_version".
    pub by_engine_version: HashMap<String, u64>,
}

/// Trait for cache backends.
///
/// Designed for future Postgres/Redis implementations alongside the
/// initial SQLite backend.
#[async_trait::async_trait]
pub trait CacheBackend: Send + Sync {
    /// Retrieve a cached entry if it exists and engine version matches.
    async fn get(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
    ) -> Result<Option<serde_json::Value>, CacheError>;

    /// Retrieve multiple entries in a single operation.
    ///
    /// Returns a map of key → data for entries found with matching version.
    async fn get_batch(
        &self,
        keys: &[String],
        task: &str,
        engine_version: &str,
    ) -> Result<HashMap<String, serde_json::Value>, CacheError>;

    /// Store a single entry.
    async fn put(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
        ba_version: &str,
        data: &serde_json::Value,
    ) -> Result<(), CacheError>;

    /// Store multiple entries in a single transaction.
    async fn put_batch(
        &self,
        entries: &[(String, serde_json::Value)],
        task: &str,
        engine_version: &str,
        ba_version: &str,
    ) -> Result<(), CacheError>;

    /// Delete specific entries by key and task. Returns count deleted.
    async fn delete_batch(&self, keys: &[String], task: &str) -> Result<usize, CacheError>;

    /// Return cache statistics.
    async fn stats(&self) -> Result<CacheStats, CacheError>;
}
