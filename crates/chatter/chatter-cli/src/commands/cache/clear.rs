//! Selective or full cache clearing for the validation cache.
//!
//! Supports two modes: `--all` removes every entry in the database, and
//! `--prefix <PATH>` removes only entries whose path starts with the given string.
//! Prefix mode is the typical choice after a corpus changes — it avoids invalidating
//! results for unrelated corpora. Both modes support `--dry-run` to preview what would
//! be removed.

use std::path::PathBuf;
use talkbank_transform::UnifiedCache;

/// Clear validation cache entries for a more reproducible `talkbank validate` run.
///
/// The CLI caches validation results by file path so rerunning the same file is cheap even when
/// the CHAT file format manual describes expensive validations like `%wor` alignment. This command
/// lets operators purge those entries entirely (`--all`) or just remove the ones matching a
/// directory/tier prefix (`--prefix <PATH>`), which is useful when a corpus changes or the manual’s
/// alignment rules evolve.
///
/// # Arguments
///
/// * `all` - Clear all cache entries
/// * `prefix` - Clear only entries matching this path prefix
/// * `dry_run` - Show what would be cleared without actually clearing
///
/// # Errors
///
/// Exits with code 1 if:
/// - Both `all` and `prefix` are specified
/// - Neither `all` nor `prefix` are specified
/// - Cache access fails
pub fn cache_clear(all: bool, prefix: Option<PathBuf>, dry_run: bool) {
    // Validate arguments
    if all && prefix.is_some() {
        eprintln!("Error: Cannot specify both --all and --prefix");
        std::process::exit(1);
    }
    if !all && prefix.is_none() {
        eprintln!("Error: Must specify either --all or --prefix <PATH>");
        std::process::exit(1);
    }

    let cache = match UnifiedCache::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to open cache: {}", e);
            std::process::exit(1);
        }
    };

    if all {
        // Clear all entries
        let count = match cache.stats() {
            Ok(stats) => stats.total_entries,
            Err(e) => {
                eprintln!("Error: Failed to read cache stats: {}", e);
                std::process::exit(1);
            }
        };

        if dry_run {
            println!("Would clear {} cache entries (dry-run)", count);
        } else {
            if let Err(e) = cache.clear_all() {
                eprintln!("Error: Failed to clear cache: {}", e);
                std::process::exit(1);
            }
            println!("Cleared {} cache entries", count);
        }
    } else if let Some(prefix_path) = prefix {
        // Clear entries matching prefix
        let prefix_str = match prefix_path.to_str() {
            Some(s) => s.to_string(),
            None => {
                eprintln!("Error: Invalid UTF-8 in path");
                std::process::exit(1);
            }
        };

        if dry_run {
            // For dry-run, we need to count how many would be cleared
            // UnifiedCache doesn't have a count_prefix method, so we just show the prefix
            println!(
                "Would clear cache entries matching prefix '{}' (dry-run)",
                prefix_str
            );
        } else {
            match cache.clear_prefix(&prefix_str) {
                Ok(count) => {
                    println!(
                        "Cleared {} cache entries matching prefix '{}'",
                        count, prefix_str
                    );
                }
                Err(e) => {
                    eprintln!("Error: Failed to clear cache: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
