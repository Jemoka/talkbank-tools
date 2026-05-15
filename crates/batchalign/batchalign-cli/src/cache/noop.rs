//! No-op cache backend that always misses.
//!
//! Used for text NLP tasks (morphosyntax, utseg, translation) where
//! re-inference with warm Stanza workers (~4ms/sentence batched) is
//! faster than SQLite lookups on a growing cache database.
//!
//! Benchmarked 2026-03-28: 6% hit rate on corpus reruns, 2.5s lookup
//! overhead per 25-file window vs ~0.1s saved inference. The cache
//! was net-negative for batch workloads.

use std::collections::HashMap;

use super::CacheError;
use super::backend::{CacheBackend, CacheStats};

/// A cache backend that never stores or returns anything.
pub(super) struct NoopBackend;

#[async_trait::async_trait]
impl CacheBackend for NoopBackend {
    async fn get(
        &self,
        _key: &str,
        _task: &str,
        _engine_version: &str,
    ) -> Result<Option<serde_json::Value>, CacheError> {
        Ok(None)
    }

    async fn get_batch(
        &self,
        _keys: &[String],
        _task: &str,
        _engine_version: &str,
    ) -> Result<HashMap<String, serde_json::Value>, CacheError> {
        Ok(HashMap::new())
    }

    async fn put(
        &self,
        _key: &str,
        _task: &str,
        _engine_version: &str,
        _ba_version: &str,
        _data: &serde_json::Value,
    ) -> Result<(), CacheError> {
        Ok(())
    }

    async fn put_batch(
        &self,
        _entries: &[(String, serde_json::Value)],
        _task: &str,
        _engine_version: &str,
        _ba_version: &str,
    ) -> Result<(), CacheError> {
        Ok(())
    }

    async fn delete_batch(&self, _keys: &[String], _task: &str) -> Result<usize, CacheError> {
        Ok(0)
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        Ok(CacheStats {
            location: "(no cache)".into(),
            size_bytes: 0,
            total_entries: 0,
            by_task: HashMap::new(),
            by_engine_version: HashMap::new(),
        })
    }
}
