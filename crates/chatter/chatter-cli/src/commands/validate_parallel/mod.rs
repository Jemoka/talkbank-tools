//! Parallel directory validation with streaming progress and caching.
//!
//! This module is split into:
//! - [`runtime`] for the standard streamed validation flow
//! - [`audit`] for JSONL audit sweeps
//! - [`shared`] for cache and fallback helpers

mod audit;
mod renderer;
mod runtime;
mod shared;

use std::path::PathBuf;

use crate::cli::OutputFormat;
use crate::ui::Theme;
use talkbank_transform::validation_runner::{ParserKind, ValidationStatsSnapshot};

/// Whether alignment-sensitive validation should run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentValidationMode {
    /// Run alignment-sensitive invariants.
    Check,
    /// Skip alignment-sensitive invariants.
    Skip,
}

impl AlignmentValidationMode {
    /// Convert the legacy boolean flag into the typed alignment mode.
    pub fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::Check } else { Self::Skip }
    }

    /// Returns `true` when alignment validation should run.
    pub fn enabled(self) -> bool {
        matches!(self, Self::Check)
    }
}

/// Whether roundtrip validation should run after the main validation pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundtripValidationMode {
    /// Run the roundtrip check.
    Check,
    /// Skip the roundtrip check.
    Skip,
}

impl RoundtripValidationMode {
    /// Convert the legacy boolean flag into the typed roundtrip mode.
    pub fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::Check } else { Self::Skip }
    }

    /// Returns `true` when roundtrip validation should run.
    pub fn enabled(self) -> bool {
        matches!(self, Self::Check)
    }
}

/// How directory traversal should treat the provided path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationTraversalMode {
    /// Recurse into directories.
    Recursive,
    /// Treat the path as one file target.
    SingleFile,
}

impl ValidationTraversalMode {
    /// Convert the legacy recursive flag into the typed traversal mode.
    pub fn from_recursive(recursive: bool) -> Self {
        if recursive {
            Self::Recursive
        } else {
            Self::SingleFile
        }
    }
}

/// Cache policy for one validation run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheRefreshMode {
    /// Reuse cached entries when they are valid.
    ReuseExisting,
    /// Clear cached entries before validating.
    ForceRefresh,
}

impl CacheRefreshMode {
    /// Convert the legacy force flag into the typed cache-refresh mode.
    pub fn from_force(force: bool) -> Self {
        if force {
            Self::ForceRefresh
        } else {
            Self::ReuseExisting
        }
    }

    /// Returns `true` when cache entries should be cleared before validation.
    pub fn should_clear_cache(self) -> bool {
        matches!(self, Self::ForceRefresh)
    }
}

/// Which interactive surface should own the streamed validation run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationInterface {
    /// Use the standard stdout/stderr renderer.
    Plain,
    /// Use the ratatui streaming interface.
    Tui,
}

impl ValidationInterface {
    /// Convert the legacy TUI flag into the typed interface mode.
    pub fn from_tui(enabled: bool) -> Self {
        if enabled { Self::Tui } else { Self::Plain }
    }

    /// Returns `true` when the run should use the streaming TUI.
    pub fn uses_tui(self) -> bool {
        matches!(self, Self::Tui)
    }
}

/// Streamed output settings for non-audit validation runs.
#[derive(Clone, Debug)]
pub struct StreamingValidationOutput {
    /// Output format for streamed results and the final summary.
    pub format: OutputFormat,
    /// Whether to suppress non-error textual output.
    pub quiet: bool,
    /// Which interactive surface should render the stream.
    pub interface: ValidationInterface,
    /// Color theme for TUI mode.
    pub theme: Theme,
}

/// Presentation mode for one validation run.
#[derive(Clone, Debug)]
pub enum ValidationPresentation {
    /// Stream events through text, JSON, or TUI output.
    Streaming(StreamingValidationOutput),
    /// Write a JSONL audit file without cache writes for new results.
    Audit {
        /// Output path for JSONL audit records.
        output_path: PathBuf,
    },
}

/// Validation-specific rules and parser choices.
#[derive(Clone, Copy, Debug)]
pub struct ValidationRules {
    /// Whether alignment-sensitive validation should run.
    pub alignment: AlignmentValidationMode,
    /// Whether roundtrip validation should run after the main pass.
    pub roundtrip: RoundtripValidationMode,
    /// Which parser backend should power validation.
    pub parser_kind: ParserKind,
    /// Enable strict cross-utterance linker validation (E351-E355).
    pub strict_linkers: bool,
}

/// Execution policy for one validation run.
#[derive(Clone, Copy, Debug)]
pub struct ValidationExecution {
    /// Cache refresh policy for the target path.
    pub cache_refresh: CacheRefreshMode,
    /// Optional worker-count override.
    pub jobs: Option<usize>,
    /// Optional cap on the number of streamed errors before cancellation.
    pub max_errors: Option<usize>,
}

/// Runtime options for parallel directory validation.
#[derive(Clone, Debug)]
pub struct ValidateDirectoryOptions {
    /// Validation rules and parser choices.
    pub rules: ValidationRules,
    /// Directory traversal policy for the target path.
    pub traversal: ValidationTraversalMode,
    /// Execution policy for cache and worker usage.
    pub execution: ValidationExecution,
    /// Presentation mode for the validation stream.
    pub presentation: ValidationPresentation,
    /// Error codes to suppress (uppercased, e.g., ["E726", "E727"]).
    pub suppress: Vec<String>,
}

/// Validate an explicit list of CHAT files using the parallel
/// streaming pipeline (worker pool, progress events, renderer, TUI).
///
/// **The single CLI entry point for `chatter validate` work.** Both
/// `chatter validate dir/` and `chatter validate a.cha b.cha c.cha`
/// resolve their input arguments to a flat .cha file list and call
/// here. The unified path replaces a previous fork where multi-file
/// invocations took a separate per-file code path with noisier output
/// and no progress bar.
///
/// `summary_label` is cosmetic — used as the `directory` field in
/// JSON summaries and the header in text output. Use the first input
/// arg, or a representative parent directory.
pub fn validate_paths_parallel(
    files: Vec<PathBuf>,
    summary_label: PathBuf,
    options: ValidateDirectoryOptions,
) -> ValidationStatsSnapshot {
    if let ValidationPresentation::Audit { output_path } = &options.presentation {
        // Audit mode is currently directory-oriented; its file walk
        // logic is in `audit::run_audit_mode` which expects a path,
        // not a file list. Route via the summary label so audit
        // continues to work for the directory case (its only real
        // caller). Multi-file audit is out of scope for this fix.
        return audit::run_audit_mode(
            &summary_label,
            output_path,
            options.rules.alignment.enabled(),
            options.execution.cache_refresh,
        );
    }

    runtime::run_validation_runtime(files, summary_label, options)
}
