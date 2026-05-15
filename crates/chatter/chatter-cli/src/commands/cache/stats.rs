//! Cache statistics display (text or JSON).
//!
//! Reports entry count, database size, last-modified timestamp, and cache directory
//! path. The `--json` flag emits a [`CacheStatistics`] struct for machine consumption,
//! useful in CI dashboards or monitoring scripts that track cache freshness.

use serde::{Deserialize, Serialize};
use talkbank_transform::UnifiedCache;

/// Serializable cache statistics in the format emitted by `talkbank cache stats --json`.
///
/// Fields align with the cache health discussion in the manual’s File Format section so automation
/// scripts can ensure cached validation data is fresh relative to the referenced CHAT files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    /// Total number of entries in the cache
    pub total_entries: usize,
    /// Cache directory path
    pub cache_dir: String,
    /// Cache database size in bytes
    pub cache_size_bytes: u64,
    /// Last modification timestamp (ISO 8601)
    pub last_modified: String,
}

/// Display cache statistics so operators can report cache health before running heavy validations.
///
/// The manual encourages periodic cache inspections when the corpus mutates; this command emits either
/// a human-friendly table or JSON that matches the schema described in the documentation.
pub fn cache_stats(json: bool) {
    let cache = match UnifiedCache::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to open cache: {}", e);
            std::process::exit(1);
        }
    };

    // Get cache stats from database
    let stats = match cache.stats() {
        Ok(stats) => stats,
        Err(e) => {
            eprintln!("Error: Failed to read cache stats: {}", e);
            std::process::exit(1);
        }
    };

    // Get cache directory and file size
    let cache_dir = stats.cache_dir.to_string_lossy().to_string();
    let cache_db_path = stats.cache_dir.join("talkbank-cache.db");

    let (cache_size_bytes, last_modified) = if cache_db_path.exists() {
        match std::fs::metadata(&cache_db_path) {
            Ok(metadata) => {
                let size = metadata.len();
                match metadata.modified() {
                    Ok(time) => match time.duration_since(std::time::UNIX_EPOCH) {
                        Ok(duration) => {
                            let secs = duration.as_secs();
                            let datetime = format_unix_timestamp(secs);
                            (size, datetime)
                        }
                        Err(_) => (size, current_timestamp()),
                    },
                    Err(_) => (size, current_timestamp()),
                }
            }
            Err(_) => (0, current_timestamp()),
        }
    } else {
        (0, current_timestamp())
    };

    if json {
        let statistics = CacheStatistics {
            total_entries: stats.total_entries,
            cache_dir,
            cache_size_bytes,
            last_modified,
        };

        match serde_json::to_string_pretty(&statistics) {
            Ok(json_str) => println!("{}", json_str),
            Err(e) => {
                eprintln!("Error: Failed to serialize JSON: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("Cache Statistics");
        println!("================");
        println!();
        println!("Cache Directory: {}", cache_dir);
        println!(
            "Cache Size:      {:.1} MB",
            cache_size_bytes as f64 / 1024.0 / 1024.0
        );
        println!("Total Entries:   {}", stats.total_entries);
        println!("Last Modified:   {}", last_modified);
    }
}

/// Format Unix timestamp as ISO 8601 string (RFC 3339).
fn format_unix_timestamp(secs: u64) -> String {
    use chrono::{DateTime, Utc};
    let datetime = match DateTime::from_timestamp(secs as i64, 0) {
        Some(dt) => dt,
        None => Utc::now(),
    };
    datetime.to_rfc3339()
}

/// Get current timestamp in ISO 8601 format (RFC 3339).
fn current_timestamp() -> String {
    use chrono::Utc;
    Utc::now().to_rfc3339()
}
