//! Cache management commands (`stats`, `clear`).
//!
//! The validation cache (`~/.cache/talkbank-tools/talkbank-cache.db`) stores per-file
//! validation outcomes keyed by path and content hash. It currently holds results for
//! 95,000+ files, so clearing it is expensive — these commands give operators
//! fine-grained control (prefix-scoped clearing, dry-run mode) to avoid unnecessary
//! revalidation work.

use crate::cli::CacheCommands;

pub mod clear;
pub mod stats;

pub use clear::cache_clear;
pub use stats::cache_stats;

/// Dispatch one `chatter cache` subcommand to its concrete implementation.
pub fn run_cache_command(command: CacheCommands) {
    match command {
        CacheCommands::Stats { json } => cache_stats(json),
        CacheCommands::Clear {
            all,
            prefix,
            dry_run,
        } => cache_clear(all, prefix, dry_run),
    }
}
