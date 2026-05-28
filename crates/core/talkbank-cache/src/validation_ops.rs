//! Validation cache read/write operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use sqlx::SqlitePool;
use std::path::Path;

use super::cache_utils::{get_cache_key_with_suffix, get_content_hash, now_secs};
use super::error::CacheError;

const CACHE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get cached validation result: `Some(true)` = valid, `Some(false)` = invalid, `None` = not cached.
pub async fn get_validation(pool: &SqlitePool, path: &Path, check_alignment: bool) -> Option<bool> {
    let key = get_cache_key_with_suffix(path, "validation");
    let current_hash = get_content_hash(path).ok()?;
    let alignment_val: i32 = if check_alignment { 1 } else { 0 };

    // Query exactly one alignment mode; callers ask for either aligned or
    // unaligned validation, and both may coexist for the same file hash.
    let row = sqlx::query_as::<_, (String, i64)>(
        "SELECT content_hash, is_valid FROM file_cache
         WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind IS NULL",
    )
    .bind(&key)
    .bind(CACHE_VERSION)
    .bind(alignment_val)
    .fetch_optional(pool)
    .await
    .ok()?;

    let (cached_hash, is_valid) = row?;

    // Invalidate if content changed.
    if cached_hash != current_hash {
        return None;
    }

    Some(is_valid != 0)
}

/// Store validation result as pass/fail.
pub async fn set_validation(
    pool: &SqlitePool,
    path: &Path,
    check_alignment: bool,
    valid: bool,
) -> Result<(), CacheError> {
    let key = get_cache_key_with_suffix(path, "validation");
    let content_hash = get_content_hash(path)?;
    let path_str = path.to_string_lossy().to_string();
    let alignment_val: i32 = if check_alignment { 1 } else { 0 };
    let valid_val: i32 = if valid { 1 } else { 0 };
    let now = now_secs()? as i64;

    sqlx::query(
        "INSERT OR REPLACE INTO file_cache
         (path_hash, file_path, content_hash, version, cached_at, check_alignment, is_valid,
          roundtrip_tested, roundtrip_passed, parser_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, NULL)",
    )
    .bind(&key)
    .bind(&path_str)
    .bind(&content_hash)
    .bind(CACHE_VERSION)
    .bind(now)
    .bind(alignment_val)
    .bind(valid_val)
    .execute(pool)
    .await
    .map_err(|source| CacheError::Database { source })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("open sqlite in-memory");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn get_validation_uses_alignment_dimension_in_lookup() {
        let pool = test_pool().await;

        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("sample.cha");
        std::fs::write(
            &file_path,
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|demo|CHI|2;00.00|||Target_Child|||\n*CHI:\thello .\n@End\n",
        )
        .expect("write test chat file");

        // Cache contradictory outcomes for the same file but different alignment modes.
        set_validation(&pool, &file_path, false, false)
            .await
            .expect("cache unaligned result");
        set_validation(&pool, &file_path, true, true)
            .await
            .expect("cache aligned result");

        let aligned = get_validation(&pool, &file_path, true).await;
        let unaligned = get_validation(&pool, &file_path, false).await;

        assert_eq!(
            aligned,
            Some(true),
            "aligned lookup should read the aligned cache row"
        );
        assert_eq!(
            unaligned,
            Some(false),
            "unaligned lookup should read the unaligned cache row"
        );
    }

    #[tokio::test]
    async fn set_validation_replaces_existing_row_for_same_key() {
        let pool = test_pool().await;

        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("sample.cha");
        std::fs::write(
            &file_path,
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|demo|CHI|2;00.00|||Target_Child|||\n*CHI:\thello .\n@End\n",
        )
        .expect("write test chat file");

        set_validation(&pool, &file_path, false, false)
            .await
            .expect("cache first result");
        set_validation(&pool, &file_path, false, true)
            .await
            .expect("replace cached result");

        let key = get_cache_key_with_suffix(&file_path, "validation");
        let row_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM file_cache
             WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind IS NULL",
        )
        .bind(&key)
        .bind(CACHE_VERSION)
        .bind(0_i32)
        .fetch_one(&pool)
        .await
        .expect("count validation rows");

        assert_eq!(
            row_count.0, 1,
            "cache should keep exactly one row per validation key"
        );
        assert_eq!(
            get_validation(&pool, &file_path, false).await,
            Some(true),
            "latest validation result should win"
        );
    }
}
