//! Validation event types and status enums
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use talkbank_model::ParseError;
// ParseError used in ErrorEvent below

/// File validation status (without error details - those stream separately)
#[derive(Debug, Clone)]
pub enum FileStatus {
    /// File passed validation (and roundtrip, if enabled).
    Valid {
        /// Whether the result came from the cache.
        cache_hit: bool,
    },
    /// File has validation errors.
    Invalid {
        /// Number of errors with `Severity::Error`.
        error_count: usize,
        /// Whether the result came from the cache.
        cache_hit: bool,
    },
    /// Validation passed but roundtrip failed.
    RoundtripFailed {
        /// Whether the result came from the cache.
        cache_hit: bool,
        /// Human-readable description of the failure.
        reason: String,
    },
    /// File could not be parsed at all.
    ParseError {
        /// Human-readable parse error message.
        message: String,
    },
    /// File could not be read from disk.
    ReadError {
        /// Human-readable I/O error message.
        message: String,
    },
}

/// Validation statistics with atomic counters (lock-free)
#[derive(Debug)]
pub struct ValidationStats {
    /// Total number of `.cha` files to validate (set once at start).
    pub total_files: usize,
    valid_files: AtomicUsize,
    invalid_files: AtomicUsize,
    cache_hits: AtomicUsize,
    cache_misses: AtomicUsize,
    parse_errors: AtomicUsize,
    roundtrip_passed: AtomicUsize,
    roundtrip_failed: AtomicUsize,
    cancelled: AtomicBool,
}

impl ValidationStats {
    /// Create new stats for a validation run over the given number of files.
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            valid_files: AtomicUsize::new(0),
            invalid_files: AtomicUsize::new(0),
            cache_hits: AtomicUsize::new(0),
            cache_misses: AtomicUsize::new(0),
            parse_errors: AtomicUsize::new(0),
            roundtrip_passed: AtomicUsize::new(0),
            roundtrip_failed: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Record that a file passed validation.
    pub fn record_valid_file(&self) {
        self.valid_files.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file failed validation.
    pub fn record_invalid_file(&self) {
        self.invalid_files.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file could not be parsed.
    pub fn record_parse_error(&self) {
        self.parse_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a result was served from cache.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file was not in cache and required parsing.
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file passed the roundtrip test.
    pub fn record_roundtrip_passed(&self) {
        self.roundtrip_passed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a file failed the roundtrip test.
    pub fn record_roundtrip_failed(&self) {
        self.roundtrip_failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark the validation run as cancelled by the user.
    pub fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Get current stats snapshot (for reporting)
    pub fn snapshot(&self) -> ValidationStatsSnapshot {
        ValidationStatsSnapshot {
            total_files: self.total_files,
            valid_files: self.valid_files.load(Ordering::Relaxed),
            invalid_files: self.invalid_files.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
            roundtrip_passed: self.roundtrip_passed.load(Ordering::Relaxed),
            roundtrip_failed: self.roundtrip_failed.load(Ordering::Relaxed),
            cancelled: self.cancelled.load(Ordering::Relaxed),
        }
    }

    /// Cache hit rate as a percentage (0.0--100.0).
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_files > 0 {
            self.cache_hits.load(Ordering::Relaxed) as f64 / self.total_files as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// Snapshot of validation stats at a point in time (Clone + Send)
#[derive(Debug, Clone)]
pub struct ValidationStatsSnapshot {
    /// Total number of `.cha` files discovered.
    pub total_files: usize,
    /// Files that passed validation.
    pub valid_files: usize,
    /// Files that failed validation.
    pub invalid_files: usize,
    /// Files whose results were served from cache.
    pub cache_hits: usize,
    /// Files that required fresh parsing.
    pub cache_misses: usize,
    /// Files that could not be parsed at all.
    pub parse_errors: usize,
    /// Files that passed the roundtrip test.
    pub roundtrip_passed: usize,
    /// Files that failed the roundtrip test.
    pub roundtrip_failed: usize,
    /// Whether the run was cancelled before completion.
    pub cancelled: bool,
}

impl ValidationStatsSnapshot {
    /// Cache hit rate as a percentage (0.0--100.0).
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_files > 0 {
            self.cache_hits as f64 / self.total_files as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// Message sent when errors are discovered
#[derive(Debug, Clone)]
pub struct ErrorEvent {
    /// Path to the file that produced errors.
    pub path: PathBuf,
    /// Parse and/or validation errors for the file.
    pub errors: Vec<ParseError>,
    /// Original source text (needed for miette rendering).
    pub source: Arc<str>,
}

/// Message sent when file completes
#[derive(Debug, Clone)]
pub struct FileCompleteEvent {
    /// Path to the completed file.
    pub path: PathBuf,
    /// Validation result for the file.
    pub status: FileStatus,
}

/// Message sent when roundtrip test completes
#[derive(Debug, Clone)]
pub struct RoundtripEvent {
    /// Path to the file that was roundtrip-tested.
    pub path: PathBuf,
    /// Whether serialization was idempotent.
    pub passed: bool,
    /// Failure reason (e.g. "Roundtrip mismatch (serialization not idempotent)").
    pub failure_reason: Option<String>,
    /// First few differing lines between pass-1 and pass-2 output.
    pub diff: Option<String>,
}

/// Validation events streamed to caller
#[derive(Debug, Clone)]
pub enum ValidationEvent {
    /// Directory discovery started - shows user that work is beginning
    Discovering,
    /// File discovery complete and validation starting.
    Started {
        /// Number of `.cha` files found.
        total_files: usize,
    },
    /// Batch of errors discovered for a single file.
    Errors(ErrorEvent),
    /// A file finished validation.
    FileComplete(FileCompleteEvent),
    /// Roundtrip test completed for a file
    RoundtripComplete(RoundtripEvent),
    /// All files have been processed; final summary statistics.
    Finished(ValidationStatsSnapshot),
}
