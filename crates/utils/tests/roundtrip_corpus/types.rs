//! Roundtrip event types and coordinator-owned statistics.
//!
//! The roundtrip corpus harness streams per-file results from worker threads to a
//! single coordinator. That coordinator forwards user-facing events and owns
//! summary counters, so workers never need shared mutable statistics state.

#![allow(dead_code)]

use std::path::PathBuf;

/// Roundtrip events streamed as the corpus run progresses.
#[derive(Debug, Clone)]
pub enum RoundtripEvent {
    /// Testing started with the discovered file count.
    Started {
        /// Total number of `.cha` files scheduled for the run.
        total_files: usize,
    },
    /// One file finished roundtrip testing.
    FileComplete {
        /// Path to the file that finished.
        path: PathBuf,
        /// Final status for that file.
        status: FileStatus,
    },
    /// All testing finished with final summary statistics.
    Finished(RoundtripStats),
}

/// Status of a single file's roundtrip result.
#[derive(Debug, Clone)]
pub enum FileStatus {
    /// The file passed roundtrip testing.
    Passed {
        /// Whether the result came from the cache.
        cache_hit: bool,
    },
    /// The file failed roundtrip testing.
    Failed {
        /// Failure category and details.
        reason: FailureReason,
        /// Whether the result came from the cache.
        cache_hit: bool,
    },
}

impl FileStatus {
    /// Return whether this file result came from the cache.
    pub fn cache_hit(&self) -> bool {
        match self {
            FileStatus::Passed { cache_hit } | FileStatus::Failed { cache_hit, .. } => *cache_hit,
        }
    }

    /// Return whether this file passed roundtrip testing.
    pub fn passed(&self) -> bool {
        matches!(self, FileStatus::Passed { .. })
    }
}

/// Reason for roundtrip test failure.
#[derive(Debug, Clone)]
pub enum FailureReason {
    /// The file could not be read from disk.
    ReadError(String),
    /// The parser failed to parse the file.
    ParseError(String),
    /// Validation failed for the file.
    ValidationError(String),
    /// Reparsing the serialized file changed its semantics.
    SemanticMismatch(String),
}

impl std::fmt::Display for FailureReason {
    /// Render a compact human-readable description of the failure.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureReason::ReadError(error) => write!(f, "read error: {}", error),
            FailureReason::ParseError(error) => write!(f, "parse error: {}", error),
            FailureReason::ValidationError(error) => write!(f, "validation error: {}", error),
            FailureReason::SemanticMismatch(detail) => {
                write!(f, "semantic mismatch: {}", detail)
            }
        }
    }
}

/// Summary statistics for one roundtrip corpus run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoundtripStats {
    /// Total number of files discovered for the run.
    pub total_files: usize,
    /// Number of files that passed.
    pub passed: usize,
    /// Number of files that failed.
    pub failed: usize,
    /// Number of results served from cache.
    pub cache_hits: usize,
    /// Number of results computed without a cache hit.
    pub cache_misses: usize,
    /// Whether the run was cancelled before all work completed.
    pub cancelled: bool,
}

impl RoundtripStats {
    /// Create a fresh stats accumulator for a run with `total_files` scheduled files.
    pub fn for_run(total_files: usize) -> Self {
        Self {
            total_files,
            ..Self::default()
        }
    }

    /// Record one completed file result in the summary counters.
    pub fn record_file_status(&mut self, status: &FileStatus) {
        if status.passed() {
            self.passed += 1;
        } else {
            self.failed += 1;
        }

        if status.cache_hit() {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
    }

    /// Compute the cache hit rate as a percentage of total scheduled files.
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_files > 0 {
            self.cache_hits as f64 / self.total_files as f64 * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for roundtrip result accounting.

    use super::{FailureReason, FileStatus, RoundtripStats};

    /// Mixed pass/fail results should update all counters consistently.
    #[test]
    fn record_file_status_updates_summary_counts() {
        let mut stats = RoundtripStats::for_run(3);

        stats.record_file_status(&FileStatus::Passed { cache_hit: true });
        stats.record_file_status(&FileStatus::Passed { cache_hit: false });
        stats.record_file_status(&FileStatus::Failed {
            reason: FailureReason::ParseError(String::from("boom")),
            cache_hit: false,
        });

        assert_eq!(
            stats,
            RoundtripStats {
                total_files: 3,
                passed: 2,
                failed: 1,
                cache_hits: 1,
                cache_misses: 2,
                cancelled: false,
            }
        );
    }

    /// Empty runs should report a zero cache hit rate instead of dividing by zero.
    #[test]
    fn cache_hit_rate_handles_empty_runs() {
        assert_eq!(RoundtripStats::default().cache_hit_rate(), 0.0);
    }
}
