//! Cache maintenance operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use sqlx::SqlitePool;
use std::path::PathBuf;
use tracing::info;

use super::error::CacheError;

/// Clear cache entries for files under the given path prefix.
pub async fn clear_prefix(pool: &SqlitePool, prefix: &str) -> Result<usize, CacheError> {
    let prefix_path = PathBuf::from(prefix);
    let paths: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT file_path FROM file_cache")
        .fetch_all(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;

    let mut removed_entries = 0;
    for (path,) in paths {
        if PathBuf::from(&path).starts_with(&prefix_path) {
            let result = sqlx::query("DELETE FROM file_cache WHERE file_path = ?1")
                .bind(&path)
                .execute(pool)
                .await
                .map_err(|source| CacheError::Database { source })?;
            removed_entries += result.rows_affected() as usize;
        }
    }

    Ok(removed_entries)
}

/// Clear all cache entries.
pub async fn clear_all(pool: &SqlitePool) -> Result<(), CacheError> {
    sqlx::query("DELETE FROM file_cache")
        .execute(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;

    Ok(())
}

/// Purge cache entries for files that no longer exist on disk.
///
/// Returns the number of removed file entries.
pub async fn purge_nonexistent(pool: &SqlitePool) -> Result<usize, CacheError> {
    let paths: Vec<(String,)> = sqlx::query_as("SELECT file_path FROM file_cache")
        .fetch_all(pool)
        .await
        .map_err(|source| CacheError::Database { source })?;

    let mut removed_files = 0;
    for (path,) in paths {
        if !PathBuf::from(&path).exists() {
            sqlx::query("DELETE FROM file_cache WHERE file_path = ?1")
                .bind(&path)
                .execute(pool)
                .await
                .map_err(|source| CacheError::Database { source })?;
            removed_files += 1;
        }
    }

    if removed_files > 0 {
        info!(
            removed_files = removed_files,
            "Purged non-existent entries from cache"
        );
    }

    Ok(removed_files)
}
