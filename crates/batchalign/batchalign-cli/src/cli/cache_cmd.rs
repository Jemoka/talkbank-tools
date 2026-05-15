//! `batchalign3 cache` — manage analysis and media caches.

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Row};
use walkdir::WalkDir;

use crate::cli::args::{CacheAction, CacheArgs};
use crate::cli::error::CliError;

// ---------------------------------------------------------------------------
// Platform-specific cache paths (matching Python `platformdirs`)
// ---------------------------------------------------------------------------

/// Analysis cache DB path (matches Python `platformdirs.user_cache_dir`).
fn default_cache_db_path() -> PathBuf {
    default_cache_db_path_from(
        crate::runtime_paths::analysis_cache_dir_override_from_env(),
        dirs::cache_dir(),
    )
}

/// Media cache directory path (matches Python `platformdirs.user_data_dir`).
fn default_media_cache_dir() -> PathBuf {
    default_media_cache_dir_from(
        crate::runtime_paths::media_cache_dir_override_from_env(),
        dirs::data_dir(),
    )
}

fn default_cache_db_path_from(
    override_dir: Option<PathBuf>,
    platform_cache_dir: Option<PathBuf>,
) -> PathBuf {
    match override_dir {
        Some(dir) => dir.join("cache.db"),
        None => platform_cache_dir
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("batchalign3")
            .join("cache.db"),
    }
}

fn default_media_cache_dir_from(
    override_dir: Option<PathBuf>,
    platform_data_dir: Option<PathBuf>,
) -> PathBuf {
    override_dir.unwrap_or_else(|| {
        platform_data_dir
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("batchalign3")
            .join("media_cache")
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format byte count as human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Compute total file count and size for a directory.
fn dir_stats(dir: &std::path::Path) -> (u64, u64) {
    if !dir.is_dir() {
        return (0, 0);
    }
    let mut count = 0u64;
    let mut size = 0u64;
    for entry in WalkDir::new(dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            count += 1;
            size += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    (count, size)
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// Print cache statistics. Uses explicit paths for testability.
async fn print_stats(
    db_path: &std::path::Path,
    media_dir: &std::path::Path,
) -> Result<(), CliError> {
    eprintln!("Batchalign Cache Statistics");
    eprintln!("{}", "-".repeat(50));

    // Analysis cache DB
    eprintln!("Location:     {}", db_path.display());

    if !db_path.exists() {
        eprintln!("Size:         0 B");
        eprintln!("Entries:      0");
    } else {
        let db_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
        eprintln!("Size:         {}", format_bytes(db_size));

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .read_only(true);

        let mut conn = options.connect().await?;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cache_entries")
            .fetch_one(&mut conn)
            .await
            .unwrap_or((0,));
        let total = total.0;
        eprintln!("Entries:      {total}");

        if total > 0 {
            eprintln!();
            eprintln!("By task:");
            let rows = sqlx::query("SELECT task, COUNT(*) as cnt FROM cache_entries GROUP BY task")
                .fetch_all(&mut conn)
                .await?;
            for row in &rows {
                let task: String = row.try_get("task")?;
                let count: i64 = row.try_get("cnt")?;
                eprintln!("  {task:<20} {count}");
            }

            eprintln!();
            eprintln!("By task + engine version:");
            let rows = sqlx::query(
                "SELECT task, engine_version, COUNT(*) as cnt FROM cache_entries GROUP BY task, engine_version",
            )
            .fetch_all(&mut conn)
            .await?;
            for row in &rows {
                let task: String = row.try_get("task")?;
                let version: String = row.try_get("engine_version")?;
                let count: i64 = row.try_get("cnt")?;
                eprintln!("  {task} {version:<12} {count}");
            }
        }
    }

    // Media cache
    eprintln!();
    eprintln!("Media cache:  {}", media_dir.display());
    let (file_count, total_size) = dir_stats(media_dir);
    eprintln!("Files:        {file_count}");
    eprintln!("Size:         {}", format_bytes(total_size));

    Ok(())
}

// ---------------------------------------------------------------------------
// Clear
// ---------------------------------------------------------------------------

/// Clear cache entries. Uses explicit paths for testability.
async fn clear_cache(
    db_path: &std::path::Path,
    media_dir: &std::path::Path,
    all: bool,
    yes: bool,
) -> Result<(), CliError> {
    if !yes {
        let scope = if all {
            "ALL cache entries (including permanent UTR entries)"
        } else {
            "non-UTR cache entries"
        };
        eprint!("Clear {scope}? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    // Clear analysis cache
    if db_path.exists() {
        let options = SqliteConnectOptions::new().filename(db_path);
        let mut conn = options.connect().await?;

        if all {
            sqlx::query("DELETE FROM cache_entries")
                .execute(&mut conn)
                .await?;
            sqlx::query("VACUUM").execute(&mut conn).await.ok(); // VACUUM may fail in some edge cases
            eprintln!("Cleared all cache entries.");
        } else {
            let result = sqlx::query("DELETE FROM cache_entries WHERE task != 'utr_asr'")
                .execute(&mut conn)
                .await?;
            let deleted = result.rows_affected();
            eprintln!("Cleared {deleted} non-UTR cache entries.");
        }
    } else {
        eprintln!("No cache database found.");
    }

    // Clear media cache
    if media_dir.is_dir() {
        let mut removed = 0u64;
        for entry in WalkDir::new(media_dir).into_iter().flatten() {
            if entry.file_type().is_file() && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            eprintln!("Removed {removed} media cache files.");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Execute the `cache` command.
pub async fn run(args: &CacheArgs) -> Result<(), CliError> {
    let db_path = default_cache_db_path();
    let media_dir = default_media_cache_dir();

    if let Some(action) = &args.action {
        return match action {
            CacheAction::Stats => print_stats(&db_path, &media_dir).await,
            CacheAction::Clear(clear) => {
                clear_cache(&db_path, &media_dir, clear.all, clear.yes).await
            }
        };
    }

    if args.stats {
        return print_stats(&db_path, &media_dir).await;
    }
    if args.clear {
        return clear_cache(&db_path, &media_dir, args.all, args.yes).await;
    }

    eprintln!(
        "error: missing cache action. Use `batchalign3 cache stats` or `batchalign3 cache clear`."
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::ConnectOptions;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[tokio::test]
    async fn stats_missing_db() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = tmp.path().join("nonexistent.db");
        let media = tmp.path().join("nonexistent_media");
        // Should not error — just prints zeros
        print_stats(&db, &media).await.unwrap();
    }

    #[tokio::test]
    async fn stats_and_clear_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("cache.db");

        // Create a cache DB with some entries
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let mut conn = options.connect().await.unwrap();

        sqlx::query(
            "CREATE TABLE cache_entries (
                key TEXT PRIMARY KEY,
                task TEXT NOT NULL,
                engine_version TEXT NOT NULL DEFAULT '',
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cache_entries (key, task, engine_version, value) VALUES (?, ?, ?, ?)",
        )
        .bind("k1")
        .bind("morphosyntax")
        .bind("1.8.2")
        .bind("data1")
        .execute(&mut conn)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cache_entries (key, task, engine_version, value) VALUES (?, ?, ?, ?)",
        )
        .bind("k2")
        .bind("morphosyntax")
        .bind("1.8.2")
        .bind("data2")
        .execute(&mut conn)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cache_entries (key, task, engine_version, value) VALUES (?, ?, ?, ?)",
        )
        .bind("k3")
        .bind("utr_asr")
        .bind("0.3.0")
        .bind("data3")
        .execute(&mut conn)
        .await
        .unwrap();
        drop(conn);

        let media_dir = tmp.path().join("media_cache");
        std::fs::create_dir_all(&media_dir).unwrap();

        // Stats should work
        print_stats(&db_path, &media_dir).await.unwrap();

        // Clear non-UTR (should remove 2, keep 1)
        clear_cache(&db_path, &media_dir, false, true)
            .await
            .unwrap();

        let options = SqliteConnectOptions::new().filename(&db_path);
        let mut conn = options.connect().await.unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cache_entries")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 1, "UTR entry should remain");
        drop(conn);

        // Clear all (should remove the UTR entry)
        clear_cache(&db_path, &media_dir, true, true).await.unwrap();

        let options = SqliteConnectOptions::new().filename(&db_path);
        let mut conn = options.connect().await.unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cache_entries")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn clear_all() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("cache.db");

        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let mut conn = options.connect().await.unwrap();

        sqlx::query(
            "CREATE TABLE cache_entries (
                key TEXT PRIMARY KEY,
                task TEXT NOT NULL,
                engine_version TEXT NOT NULL DEFAULT '',
                value TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cache_entries (key, task, engine_version, value) VALUES (?, ?, ?, ?)",
        )
        .bind("k1")
        .bind("utr_asr")
        .bind("0.3.0")
        .bind("data")
        .execute(&mut conn)
        .await
        .unwrap();
        drop(conn);

        let media_dir = tmp.path().join("media_cache");
        clear_cache(&db_path, &media_dir, true, true).await.unwrap();

        let options = SqliteConnectOptions::new().filename(&db_path);
        let mut conn = options.connect().await.unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cache_entries")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }

    #[test]
    fn media_cache_stats_missing_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = tmp.path().join("cache.db");
        let media = tmp.path().join("no_such_dir");
        // Should not error
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(print_stats(&db, &media))
            .unwrap();
    }

    #[test]
    fn media_cache_stats_counts_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let media = tmp.path().join("media_cache");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::write(media.join("a.wav"), b"fake wav data").unwrap();
        std::fs::write(media.join("b.wav"), b"more data here").unwrap();

        let (count, size) = dir_stats(&media);
        assert_eq!(count, 2);
        assert!(size > 0);
    }

    #[test]
    fn default_cache_db_path_prefers_explicit_override() {
        let resolved = default_cache_db_path_from(
            Some(PathBuf::from("/tmp/analysis-cache")),
            Some(PathBuf::from("/tmp/platform-cache")),
        );

        assert_eq!(resolved, PathBuf::from("/tmp/analysis-cache/cache.db"));
    }

    #[test]
    fn default_media_cache_dir_prefers_explicit_override() {
        let resolved = default_media_cache_dir_from(
            Some(PathBuf::from("/tmp/media-cache")),
            Some(PathBuf::from("/tmp/platform-data")),
        );

        assert_eq!(resolved, PathBuf::from("/tmp/media-cache"));
    }
}
