//! SQLite WAL backend for the utterance cache.
//!
//! Compatible with the Python `CacheManager`: same DB file, same schema,
//! same key formulas. Both Rust and Python can read/write the same cache DB
//! concurrently via WAL mode.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::cache::CacheError;
use crate::cache::backend::{CacheBackend, CacheStats};

/// Maximum parameters per SQLite IN clause (keep margin for other params).
const CHUNK_SIZE: usize = 900;

/// SQLite WAL cache backend.
pub struct SqliteBackend {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl SqliteBackend {
    /// Open (or create) the cache database.
    ///
    /// If `cache_dir` is `None`, uses the platform cache directory:
    /// - macOS: `~/Library/Caches/batchalign3/`
    /// - Linux: `~/.cache/batchalign3/`
    ///
    /// This matches Python's `platformdirs.user_cache_dir("batchalign3", "batchalign3")`.
    pub async fn open(cache_dir: Option<PathBuf>) -> Result<Self, CacheError> {
        let cache_dir = match cache_dir {
            Some(dir) => dir,
            None => default_cache_dir()?,
        };

        std::fs::create_dir_all(&cache_dir)?;
        let db_path = cache_dir.join("cache.db");

        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_millis(10_000))
            .synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./cache_migrations").run(&pool).await?;

        Ok(Self { pool, db_path })
    }

    /// Path to the database file (for diagnostics).
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

/// Resolve the platform-default cache directory.
fn default_cache_dir() -> Result<PathBuf, CacheError> {
    default_cache_dir_from(
        crate::runtime_paths::analysis_cache_dir_override_from_env(),
        dirs::cache_dir(),
    )
}

fn default_cache_dir_from(
    override_dir: Option<PathBuf>,
    platform_cache_dir: Option<PathBuf>,
) -> Result<PathBuf, CacheError> {
    override_dir
        .or_else(|| platform_cache_dir.map(|d| d.join("batchalign3")))
        .ok_or(CacheError::NoCacheDir)
}

#[async_trait::async_trait]
impl CacheBackend for SqliteBackend {
    async fn get(
        &self,
        key: &str,
        task: &str,
        engine_version: &str,
    ) -> Result<Option<serde_json::Value>, CacheError> {
        let row = sqlx::query(
            "SELECT data, engine_version FROM cache_entries WHERE key = ? AND task = ?",
        )
        .bind(key)
        .bind(task)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => {
                tracing::debug!(
                    task,
                    key_prefix = &key[..key.len().min(16)],
                    "cache MISS (not found)"
                );
                Ok(None)
            }
            Some(row) => {
                let data_blob: String = row.try_get("data")?;
                let stored_version: String = row.try_get("engine_version")?;

                if stored_version != engine_version {
                    tracing::debug!(
                        task,
                        key_prefix = &key[..key.len().min(16)],
                        stored = %stored_version,
                        expected = %engine_version,
                        "cache MISS (version mismatch)"
                    );
                    Ok(None)
                } else {
                    tracing::debug!(task, key_prefix = &key[..key.len().min(16)], "cache HIT");
                    let value: serde_json::Value = serde_json::from_str(&data_blob)?;
                    Ok(Some(value))
                }
            }
        }
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

        for chunk in keys.chunks(CHUNK_SIZE) {
            let mut qb = sqlx::QueryBuilder::new(
                "SELECT key, data, engine_version FROM cache_entries WHERE key IN (",
            );

            let mut separated = qb.separated(", ");
            for k in chunk {
                separated.push_bind(k.clone());
            }
            separated.push_unseparated(") AND task = ");
            qb.push_bind(task.to_string());

            let rows = qb.build().fetch_all(&self.pool).await?;

            for row in rows {
                let key: String = row.try_get("key")?;
                let data_blob: String = row.try_get("data")?;
                let stored_version: String = row.try_get("engine_version")?;

                if stored_version != engine_version {
                    tracing::debug!(
                        task,
                        key_prefix = &key[..key.len().min(16)],
                        stored = %stored_version,
                        expected = %engine_version,
                        "cache MISS (version mismatch)"
                    );
                } else {
                    let value: serde_json::Value = serde_json::from_str(&data_blob)?;
                    results.insert(key, value);
                }
            }
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
        let created_at = Utc::now().to_rfc3339();
        let data_blob = serde_json::to_string(data)?;

        sqlx::query(
            "INSERT OR REPLACE INTO cache_entries \
             (key, task, engine_version, batchalign_version, created_at, data) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(key)
        .bind(task)
        .bind(engine_version)
        .bind(ba_version)
        .bind(created_at)
        .bind(data_blob)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn put_batch(
        &self,
        entries: &[(String, serde_json::Value)],
        task: &str,
        engine_version: &str,
        ba_version: &str,
    ) -> Result<(), CacheError> {
        if entries.is_empty() {
            return Ok(());
        }

        let created_at = Utc::now().to_rfc3339();

        let mut tx = self.pool.begin().await?;
        for (key, data) in entries {
            let data_blob = serde_json::to_string(data)?;
            sqlx::query(
                "INSERT OR REPLACE INTO cache_entries \
                 (key, task, engine_version, batchalign_version, created_at, data) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(key)
            .bind(task)
            .bind(engine_version)
            .bind(ba_version)
            .bind(&created_at)
            .bind(data_blob)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(())
    }

    async fn delete_batch(&self, keys: &[String], task: &str) -> Result<usize, CacheError> {
        if keys.is_empty() {
            return Ok(0);
        }

        let mut total_deleted = 0usize;

        for chunk in keys.chunks(CHUNK_SIZE) {
            let mut qb = sqlx::QueryBuilder::new("DELETE FROM cache_entries WHERE task = ");
            qb.push_bind(task.to_string());
            qb.push(" AND key IN (");

            let mut separated = qb.separated(", ");
            for k in chunk {
                separated.push_bind(k.clone());
            }
            separated.push_unseparated(")");

            let result = qb.build().execute(&self.pool).await?;
            total_deleted += result.rows_affected() as usize;
        }

        if total_deleted > 0 {
            tracing::debug!(
                task,
                count = total_deleted,
                "cache self-correction: deleted stale entries"
            );
        }

        Ok(total_deleted)
    }

    async fn stats(&self) -> Result<CacheStats, CacheError> {
        // File sizes (no pool needed for stat).
        let mut size_bytes = 0u64;
        for suffix in ["", "-wal", "-shm"] {
            let path = PathBuf::from(format!("{}{suffix}", self.db_path.display()));
            if let Ok(meta) = std::fs::metadata(&path) {
                size_bytes += meta.len();
            }
        }

        let total_entries: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cache_entries")
            .fetch_one(&self.pool)
            .await?;
        let total_entries = total_entries.0 as u64;

        let mut by_task = HashMap::new();
        {
            let rows = sqlx::query("SELECT task, COUNT(*) as cnt FROM cache_entries GROUP BY task")
                .fetch_all(&self.pool)
                .await?;
            for row in rows {
                let task: String = row.try_get("task")?;
                let count: i64 = row.try_get("cnt")?;
                by_task.insert(task, count as u64);
            }
        }

        let mut by_engine_version = HashMap::new();
        {
            let rows = sqlx::query(
                "SELECT task, engine_version, COUNT(*) as cnt FROM cache_entries GROUP BY task, engine_version",
            )
            .fetch_all(&self.pool)
            .await?;
            for row in rows {
                let task: String = row.try_get("task")?;
                let version: String = row.try_get("engine_version")?;
                let count: i64 = row.try_get("cnt")?;
                by_engine_version.insert(format!("{task} {version}"), count as u64);
            }
        }

        Ok(CacheStats {
            location: self.db_path.display().to_string(),
            size_bytes,
            total_entries,
            by_task,
            by_engine_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_cache_dir_prefers_explicit_override() {
        let resolved = default_cache_dir_from(
            Some(PathBuf::from("/tmp/analysis-cache")),
            Some(PathBuf::from("/tmp/platform-cache")),
        )
        .expect("override should resolve");

        assert_eq!(resolved, PathBuf::from("/tmp/analysis-cache"));
    }

    #[test]
    fn default_cache_dir_uses_platform_cache_when_no_override() {
        let resolved = default_cache_dir_from(None, Some(PathBuf::from("/tmp/platform-cache")))
            .expect("platform cache should resolve");

        assert_eq!(resolved, PathBuf::from("/tmp/platform-cache/batchalign3"));
    }

    async fn test_backend() -> (SqliteBackend, TempDir) {
        let dir = TempDir::new().unwrap();
        let backend = SqliteBackend::open(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        (backend, dir)
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let (backend, _dir) = test_backend().await;
        let data = serde_json::json!({"mor": "n|dog", "gra": "1|0|ROOT"});

        backend
            .put("key1", "morphosyntax_v4", "stanza-1.9", "3.0.0", &data)
            .await
            .unwrap();

        let result = backend
            .get("key1", "morphosyntax_v4", "stanza-1.9")
            .await
            .unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test]
    async fn test_get_miss_not_found() {
        let (backend, _dir) = test_backend().await;
        let result = backend
            .get("missing", "morphosyntax_v4", "stanza-1.9")
            .await
            .unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_get_miss_version_mismatch() {
        let (backend, _dir) = test_backend().await;
        let data = serde_json::json!({"mor": "n|dog"});

        backend
            .put("key1", "morphosyntax_v4", "stanza-1.8", "3.0.0", &data)
            .await
            .unwrap();

        let result = backend
            .get("key1", "morphosyntax_v4", "stanza-1.9")
            .await
            .unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_put_replaces_existing() {
        let (backend, _dir) = test_backend().await;
        let data1 = serde_json::json!({"mor": "n|dog"});
        let data2 = serde_json::json!({"mor": "n|cat"});

        backend
            .put("key1", "task", "v1", "3.0.0", &data1)
            .await
            .unwrap();
        backend
            .put("key1", "task", "v1", "3.0.0", &data2)
            .await
            .unwrap();

        let result = backend.get("key1", "task", "v1").await.unwrap();
        assert_eq!(result, Some(data2));
    }

    #[tokio::test]
    async fn test_get_batch() {
        let (backend, _dir) = test_backend().await;

        for i in 0..5 {
            let data = serde_json::json!({"idx": i});
            backend
                .put(&format!("key{i}"), "task", "v1", "3.0.0", &data)
                .await
                .unwrap();
        }

        // Also put one with wrong version.
        backend
            .put("key_old", "task", "v0", "2.0.0", &serde_json::json!({}))
            .await
            .unwrap();

        let keys: Vec<String> = (0..5).map(|i| format!("key{i}")).collect();
        let mut all_keys = keys.clone();
        all_keys.push("key_old".to_string());
        all_keys.push("nonexistent".to_string());

        let results = backend.get_batch(&all_keys, "task", "v1").await.unwrap();
        assert_eq!(results.len(), 5);
        for i in 0..5 {
            let key = format!("key{i}");
            assert_eq!(results[&key], serde_json::json!({"idx": i}));
        }
    }

    #[tokio::test]
    async fn test_put_batch() {
        let (backend, _dir) = test_backend().await;

        let entries: Vec<(String, serde_json::Value)> = (0..10)
            .map(|i| (format!("key{i}"), serde_json::json!({"i": i})))
            .collect();

        backend
            .put_batch(&entries, "task", "v1", "3.0.0")
            .await
            .unwrap();

        let keys: Vec<String> = (0..10).map(|i| format!("key{i}")).collect();
        let results = backend.get_batch(&keys, "task", "v1").await.unwrap();
        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_delete_batch() {
        let (backend, _dir) = test_backend().await;

        for i in 0..5 {
            let data = serde_json::json!({"i": i});
            backend
                .put(&format!("key{i}"), "task", "v1", "3.0.0", &data)
                .await
                .unwrap();
        }

        let to_delete: Vec<String> = vec!["key1".into(), "key3".into(), "nonexistent".into()];
        let deleted = backend.delete_batch(&to_delete, "task").await.unwrap();
        assert_eq!(deleted, 2);

        // Remaining entries.
        let all_keys: Vec<String> = (0..5).map(|i| format!("key{i}")).collect();
        let results = backend.get_batch(&all_keys, "task", "v1").await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.contains_key("key0"));
        assert!(results.contains_key("key2"));
        assert!(results.contains_key("key4"));
    }

    #[tokio::test]
    async fn test_stats() {
        let (backend, _dir) = test_backend().await;

        backend
            .put(
                "k1",
                "morphosyntax_v4",
                "stanza-1.9",
                "3.0.0",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        backend
            .put(
                "k2",
                "morphosyntax_v4",
                "stanza-1.9",
                "3.0.0",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        backend
            .put("k3", "utseg", "stanza-1.9", "3.0.0", &serde_json::json!({}))
            .await
            .unwrap();

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.by_task.get("morphosyntax_v4"), Some(&2));
        assert_eq!(stats.by_task.get("utseg"), Some(&1));
        assert!(stats.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_empty_batch_operations() {
        let (backend, _dir) = test_backend().await;

        let results = backend.get_batch(&[], "task", "v1").await.unwrap();
        assert!(results.is_empty());

        backend.put_batch(&[], "task", "v1", "3.0.0").await.unwrap();

        let deleted = backend.delete_batch(&[], "task").await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_large_batch_chunking() {
        let (backend, _dir) = test_backend().await;

        // Insert more than CHUNK_SIZE entries to exercise chunking.
        let entries: Vec<(String, serde_json::Value)> = (0..1000)
            .map(|i| (format!("key{i:04}"), serde_json::json!({"i": i})))
            .collect();

        backend
            .put_batch(&entries, "task", "v1", "3.0.0")
            .await
            .unwrap();

        let keys: Vec<String> = (0..1000).map(|i| format!("key{i:04}")).collect();
        let results = backend.get_batch(&keys, "task", "v1").await.unwrap();
        assert_eq!(results.len(), 1000);
    }

    #[tokio::test]
    async fn test_cross_task_isolation() {
        let (backend, _dir) = test_backend().await;

        backend
            .put(
                "same_key",
                "task_a",
                "v1",
                "3.0.0",
                &serde_json::json!({"a": 1}),
            )
            .await
            .unwrap();
        // Same key, different task — should not collide because key is PRIMARY KEY.
        // In the Python schema, (key) is the PK, so same key with different task
        // overwrites. Let's verify this matches Python behavior.
        backend
            .put(
                "same_key",
                "task_b",
                "v1",
                "3.0.0",
                &serde_json::json!({"b": 2}),
            )
            .await
            .unwrap();

        // Python schema: key is PRIMARY KEY (alone), so second insert replaces first.
        let result = backend.get("same_key", "task_b", "v1").await.unwrap();
        assert_eq!(result, Some(serde_json::json!({"b": 2})));

        // The task_a entry was overwritten.
        let result = backend.get("same_key", "task_a", "v1").await.unwrap();
        assert_eq!(result, None);
    }
}
