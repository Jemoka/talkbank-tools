//! Tiered cache backend: moka in-memory hot layer + persistent cold backend.
//!
//! Write-through: reads check moka first, writes go to both layers.
//! SQLite (or any cold backend) remains the authoritative persistent store.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

use crate::cache::CacheError;
use crate::cache::backend::{CacheBackend, CacheStats};

/// A single hot-cache entry, storing task/version for match verification.
#[derive(Clone, Debug)]
struct HotEntry {
    task: String,
    engine_version: String,
    data: serde_json::Value,
}

/// Default maximum number of entries in the hot cache.
const DEFAULT_MAX_CAPACITY: u64 = 10_000;

/// Default time-to-idle before eviction (24 hours).
const DEFAULT_TTI: Duration = Duration::from_secs(24 * 60 * 60);

/// Tiered cache: moka in-memory hot layer wrapping a persistent cold backend.
///
/// - **Read path:** check moka → on hit, verify task+engine_version → on miss
///   or mismatch, fall through to cold → promote cold hits to moka.
/// - **Write path:** write to cold first (authoritative), then insert into moka.
/// - **Delete path:** remove from moka first, then cold.
/// - **Stats:** delegated to cold (authoritative).
pub struct TieredCacheBackend {
    hot: Cache<String, Arc<HotEntry>>,
    cold: Box<dyn CacheBackend>,
}

impl TieredCacheBackend {
    /// Create a new tiered cache wrapping the given cold backend.
    ///
    /// - `max_capacity`: maximum hot-cache entries (default 10,000 → ~5-20 MB).
    pub fn new(cold: Box<dyn CacheBackend>, max_capacity: Option<u64>) -> Self {
        let hot = Cache::builder()
            .max_capacity(max_capacity.unwrap_or(DEFAULT_MAX_CAPACITY))
            .time_to_idle(DEFAULT_TTI)
            .build();

        Self { hot, cold }
    }
}

#[async_trait::async_trait]
impl CacheBackend for TieredCacheBackend {
    async fn get(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
    ) -> Result<Option<serde_json::Value>, CacheError> {
        // Check hot cache first.
        if let Some(entry) = self.hot.get(key).await {
            if entry.task == task && entry.engine_version == engine_version {
                tracing::debug!(
                    task,
                    key_prefix = &key[..key.len().min(16)],
                    "hot cache HIT"
                );
                return Ok(Some(entry.data.clone()));
            }
            // Task or version mismatch — fall through to cold.
            tracing::debug!(
                task,
                key_prefix = &key[..key.len().min(16)],
                "hot cache MISS (stale entry)"
            );
        }

        // Fall through to cold backend.
        let result = self.cold.get(key, task, engine_version).await?;

        // Promote cold hit to hot cache.
        if let Some(ref data) = result {
            self.hot
                .insert(
                    key.to_string(),
                    Arc::new(HotEntry {
                        task: task.to_string(),
                        engine_version: engine_version.to_string(),
                        data: data.clone(),
                    }),
                )
                .await;
        }

        Ok(result)
    }

    async fn get_batch(
        &self,
        keys: &[String],
        task: &str,
        engine_version: &str,
    ) -> Result<HashMap<String, serde_json::Value>, CacheError> {
        if keys.is_empty() {
            return Ok(HashMap::new());
        }

        let mut results = HashMap::new();
        let mut cold_keys = Vec::new();

        // Collect hot hits first.
        for key in keys {
            if let Some(entry) = self.hot.get(key).await
                && entry.task == task
                && entry.engine_version == engine_version
            {
                results.insert(key.clone(), entry.data.clone());
                continue;
            }
            cold_keys.push(key.clone());
        }

        if !cold_keys.is_empty() {
            let hot_hits = results.len();

            // Query cold only for misses.
            let cold_results = self
                .cold
                .get_batch(&cold_keys, task, engine_version)
                .await?;

            // Promote cold hits to hot cache.
            for (key, data) in &cold_results {
                self.hot
                    .insert(
                        key.clone(),
                        Arc::new(HotEntry {
                            task: task.to_string(),
                            engine_version: engine_version.to_string(),
                            data: data.clone(),
                        }),
                    )
                    .await;
            }

            let cold_hits = cold_results.len();
            if hot_hits > 0 || cold_hits > 0 {
                tracing::debug!(
                    task,
                    hot_hits,
                    cold_hits,
                    total_keys = keys.len(),
                    "batch cache lookup"
                );
            }

            results.extend(cold_results);
        }

        Ok(results)
    }

    async fn put(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
        ba_version: &str,
        data: &serde_json::Value,
    ) -> Result<(), CacheError> {
        // Write to cold first (authoritative).
        self.cold
            .put(key, task, engine_version, ba_version, data)
            .await?;

        // Then populate hot cache.
        self.hot
            .insert(
                key.to_string(),
                Arc::new(HotEntry {
                    task: task.to_string(),
                    engine_version: engine_version.to_string(),
                    data: data.clone(),
                }),
            )
            .await;

        Ok(())
    }

    async fn put_batch(
        &self,
        entries: &[(String, serde_json::Value)],
        task: &str,
        engine_version: &str,
        ba_version: &str,
    ) -> Result<(), CacheError> {
        // Write to cold first (authoritative).
        self.cold
            .put_batch(entries, task, engine_version, ba_version)
            .await?;

        // Populate hot cache.
        for (key, data) in entries {
            self.hot
                .insert(
                    key.clone(),
                    Arc::new(HotEntry {
                        task: task.to_string(),
                        engine_version: engine_version.to_string(),
                        data: data.clone(),
                    }),
                )
                .await;
        }

        Ok(())
    }

    async fn delete_batch(&self, keys: &[String], task: &str) -> Result<usize, CacheError> {
        // Remove from hot first.
        for key in keys {
            self.hot.invalidate(key).await;
        }

        // Then from cold (authoritative count).
        self.cold.delete_batch(keys, task).await
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        // Delegate to cold (authoritative).
        self.cold.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::SqliteBackend;
    use tempfile::TempDir;

    async fn test_tiered() -> (TieredCacheBackend, TempDir) {
        let dir = TempDir::new().unwrap();
        let sqlite = SqliteBackend::open(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let tiered = TieredCacheBackend::new(Box::new(sqlite), Some(1000));
        (tiered, dir)
    }

    /// Build a tiered cache where we can also access the cold backend directly
    /// to verify hot-layer isolation.
    async fn test_tiered_with_cold() -> (TieredCacheBackend, SqliteBackend, TempDir) {
        let dir = TempDir::new().unwrap();
        let cold = SqliteBackend::open(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        // Open a second connection to the same DB for verification.
        let verifier = SqliteBackend::open(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let tiered = TieredCacheBackend::new(Box::new(cold), Some(1000));
        (tiered, verifier, dir)
    }

    #[tokio::test]
    async fn test_put_and_get_hot_path() {
        let (tiered, verifier, _dir) = test_tiered_with_cold().await;
        let data = serde_json::json!({"mor": "n|dog"});

        tiered
            .put("k1", "morphosyntax", "stanza-1.9", "3.0.0", &data)
            .await
            .unwrap();

        // Hot cache should serve this without hitting SQLite.
        let result = tiered
            .get("k1", "morphosyntax", "stanza-1.9")
            .await
            .unwrap();
        assert_eq!(result, Some(data.clone()));

        // Verify it also made it to cold.
        let cold_result = verifier
            .get("k1", "morphosyntax", "stanza-1.9")
            .await
            .unwrap();
        assert_eq!(cold_result, Some(data));
    }

    #[tokio::test]
    async fn test_get_cold_promotes_to_hot() {
        let (tiered, verifier, _dir) = test_tiered_with_cold().await;
        let data = serde_json::json!({"seg": [3, 7]});

        // Write directly to cold (bypassing hot).
        verifier
            .put("k1", "utseg", "stanza-1.9", "3.0.0", &data)
            .await
            .unwrap();

        // First get: cold hit, should promote to hot.
        let result = tiered.get("k1", "utseg", "stanza-1.9").await.unwrap();
        assert_eq!(result, Some(data.clone()));

        // Verify it's now in hot cache (entry exists).
        assert!(tiered.hot.get("k1").await.is_some());

        // Second get should be a hot hit.
        let result2 = tiered.get("k1", "utseg", "stanza-1.9").await.unwrap();
        assert_eq!(result2, Some(data));
    }

    #[tokio::test]
    async fn test_get_batch_mixed_hot_cold() {
        let (tiered, verifier, _dir) = test_tiered_with_cold().await;

        // Put k1 through tiered (ends up in hot + cold).
        tiered
            .put("k1", "task", "v1", "3.0.0", &serde_json::json!({"i": 1}))
            .await
            .unwrap();

        // Put k2 directly to cold only.
        verifier
            .put("k2", "task", "v1", "3.0.0", &serde_json::json!({"i": 2}))
            .await
            .unwrap();

        let keys = vec!["k1".to_string(), "k2".to_string(), "k3".to_string()];
        let results = tiered.get_batch(&keys, "task", "v1").await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results["k1"], serde_json::json!({"i": 1}));
        assert_eq!(results["k2"], serde_json::json!({"i": 2}));

        // k2 should now be promoted to hot.
        assert!(tiered.hot.get("k2").await.is_some());
    }

    #[tokio::test]
    async fn test_version_mismatch_falls_through() {
        let (tiered, verifier, _dir) = test_tiered_with_cold().await;

        // Put with v1 through tiered (hot has v1 entry).
        tiered
            .put(
                "k1",
                "task",
                "v1",
                "3.0.0",
                &serde_json::json!({"old": true}),
            )
            .await
            .unwrap();

        // Now put v2 directly to cold (simulating engine upgrade).
        verifier
            .put(
                "k1",
                "task",
                "v2",
                "3.1.0",
                &serde_json::json!({"new": true}),
            )
            .await
            .unwrap();

        // Requesting v2 should skip stale hot entry and hit cold.
        let result = tiered.get("k1", "task", "v2").await.unwrap();
        assert_eq!(result, Some(serde_json::json!({"new": true})));

        // Hot cache should now have the v2 entry.
        let hot_entry = tiered.hot.get("k1").await.unwrap();
        assert_eq!(hot_entry.engine_version, "v2");
    }

    #[tokio::test]
    async fn test_delete_batch_evicts_hot() {
        let (tiered, _dir) = test_tiered().await;

        tiered
            .put("k1", "task", "v1", "3.0.0", &serde_json::json!({"i": 1}))
            .await
            .unwrap();
        tiered
            .put("k2", "task", "v1", "3.0.0", &serde_json::json!({"i": 2}))
            .await
            .unwrap();

        let deleted = tiered
            .delete_batch(&["k1".to_string()], "task")
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        // k1 should be gone from both hot and cold.
        assert!(tiered.hot.get("k1").await.is_none());
        let result = tiered.get("k1", "task", "v1").await.unwrap();
        assert_eq!(result, None);

        // k2 should still be there.
        let result = tiered.get("k2", "task", "v1").await.unwrap();
        assert_eq!(result, Some(serde_json::json!({"i": 2})));
    }

    #[tokio::test]
    async fn test_empty_batch_operations() {
        let (tiered, _dir) = test_tiered().await;

        let results = tiered.get_batch(&[], "task", "v1").await.unwrap();
        assert!(results.is_empty());

        tiered.put_batch(&[], "task", "v1", "3.0.0").await.unwrap();

        let deleted = tiered.delete_batch(&[], "task").await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_put_batch_populates_hot() {
        let (tiered, _dir) = test_tiered().await;

        let entries: Vec<(String, serde_json::Value)> = (0..5)
            .map(|i| (format!("k{i}"), serde_json::json!({"i": i})))
            .collect();

        tiered
            .put_batch(&entries, "task", "v1", "3.0.0")
            .await
            .unwrap();

        // All should be in hot cache.
        for i in 0..5 {
            let key = format!("k{i}");
            assert!(tiered.hot.get(&key).await.is_some());
        }

        // And retrievable.
        let keys: Vec<String> = (0..5).map(|i| format!("k{i}")).collect();
        let results = tiered.get_batch(&keys, "task", "v1").await.unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_stats_delegates_to_cold() {
        let (tiered, _dir) = test_tiered().await;

        tiered
            .put(
                "k1",
                "morphosyntax",
                "stanza-1.9",
                "3.0.0",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        tiered
            .put("k2", "utseg", "stanza-1.9", "3.0.0", &serde_json::json!({}))
            .await
            .unwrap();

        let stats = tiered.stats().await.unwrap();
        // Stats come from SQLite, not moka.
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.by_task.get("morphosyntax"), Some(&1));
        assert_eq!(stats.by_task.get("utseg"), Some(&1));
        assert!(stats.size_bytes > 0);
    }
}
