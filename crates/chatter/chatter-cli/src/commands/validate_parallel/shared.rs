//! Shared fallback validation statistics for interactive exits.

use talkbank_transform::validation_runner::ValidationStatsSnapshot;

/// Return an empty stats snapshot used when an interactive run exits before completion.
pub fn empty_stats(cancelled: bool) -> ValidationStatsSnapshot {
    ValidationStatsSnapshot {
        total_files: 0,
        valid_files: 0,
        invalid_files: 0,
        cache_hits: 0,
        cache_misses: 0,
        parse_errors: 0,
        roundtrip_passed: 0,
        roundtrip_failed: 0,
        cancelled,
    }
}
