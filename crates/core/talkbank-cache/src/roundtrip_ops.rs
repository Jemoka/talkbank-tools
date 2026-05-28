//! Roundtrip test cache operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use sqlx::SqlitePool;
use std::path::Path;

use super::cache_utils::{get_cache_key_with_suffix, get_content_hash, now_secs};
use super::error::CacheError;

const CACHE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get cached roundtrip result: `Some(true)` = passed, `Some(false)` = failed, `None` = not cached.
pub async fn get_roundtrip(
    pool: &SqlitePool,
    path: &Path,
    check_alignment: bool,
    parser_kind: &str,
) -> Option<bool> {
    let key = get_cache_key_with_suffix(path, parser_kind);
    let current_hash = get_content_hash(path).ok()?;
    let alignment_val: i32 = if check_alignment { 1 } else { 0 };

    let row = sqlx::query_as::<_, (String, i64, Option<i64>)>(
        "SELECT content_hash, roundtrip_tested, roundtrip_passed
         FROM file_cache
         WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind = ?4",
    )
    .bind(&key)
    .bind(CACHE_VERSION)
    .bind(alignment_val)
    .bind(parser_kind)
    .fetch_optional(pool)
    .await
    .ok()?;

    let (cached_hash, roundtrip_tested, roundtrip_passed) = row?;

    // Invalidate if content changed.
    if cached_hash != current_hash {
        return None;
    }

    // Only return result if roundtrip was actually tested
    if roundtrip_tested == 0 {
        return None;
    }

    roundtrip_passed.map(|p| p != 0)
}

/// Store roundtrip result as pass/fail.
pub async fn set_roundtrip(
    pool: &SqlitePool,
    path: &Path,
    check_alignment: bool,
    parser_kind: &str,
    passed: bool,
) -> Result<(), CacheError> {
    let key = get_cache_key_with_suffix(path, parser_kind);
    let content_hash = get_content_hash(path)?;
    let path_str = path.to_string_lossy().to_string();
    let alignment_val: i32 = if check_alignment { 1 } else { 0 };
    let passed_val: i32 = if passed { 1 } else { 0 };
    let now = now_secs()? as i64;

    sqlx::query(
        "INSERT OR REPLACE INTO file_cache
         (path_hash, file_path, content_hash, version, cached_at, check_alignment, is_valid,
          roundtrip_tested, roundtrip_passed, parser_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9)",
    )
    .bind(&key)
    .bind(&path_str)
    .bind(&content_hash)
    .bind(CACHE_VERSION)
    .bind(now)
    .bind(alignment_val)
    .bind(passed_val) // is_valid mirrors roundtrip result
    .bind(passed_val)
    .bind(parser_kind)
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
    async fn set_roundtrip_replaces_existing_row_for_same_key() {
        let pool = test_pool().await;

        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("sample.cha");
        std::fs::write(
            &file_path,
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|demo|CHI|2;00.00|||Target_Child|||\n*CHI:\thello .\n@End\n",
        )
        .expect("write test chat file");

        set_roundtrip(&pool, &file_path, false, "tree-sitter", false)
            .await
            .expect("cache first roundtrip result");
        set_roundtrip(&pool, &file_path, false, "tree-sitter", true)
            .await
            .expect("replace roundtrip result");

        let key = get_cache_key_with_suffix(&file_path, "tree-sitter");
        let row_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM file_cache
             WHERE path_hash = ?1 AND version = ?2 AND check_alignment = ?3 AND parser_kind = ?4",
        )
        .bind(&key)
        .bind(CACHE_VERSION)
        .bind(0_i32)
        .bind("tree-sitter")
        .fetch_one(&pool)
        .await
        .expect("count roundtrip rows");

        assert_eq!(
            row_count.0, 1,
            "cache should keep exactly one row per roundtrip key"
        );
        assert_eq!(
            get_roundtrip(&pool, &file_path, false, "tree-sitter").await,
            Some(true),
            "latest roundtrip result should win"
        );
    }
}
