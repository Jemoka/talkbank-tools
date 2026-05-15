//! Shared application state, CLI arguments, and synchronization helpers for the test dashboard.

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool, mpsc::Sender};
use std::time::Instant;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use talkbank_transform::corpus::manifest::{ErrorDetail, FailureReason};
use talkbank_transform::{CorpusManifest, UnifiedCache};

use crate::test_dashboard::manifest::DashboardManifest;

/// Typed result for one file-level roundtrip run.
#[derive(Clone, Debug)]
pub struct FileTestOutcome {
    /// Canonical path to the tested CHAT file.
    pub path: PathBuf,
    /// Whether the file passed.
    pub passed: bool,
    /// Structured failure category, when the file failed.
    pub failure_reason: Option<FailureReason>,
    /// Structured failure detail, when available.
    pub error_detail: Option<ErrorDetail>,
}

impl FileTestOutcome {
    /// Construct one file outcome from execution or cache results.
    pub fn new(
        path: PathBuf,
        passed: bool,
        failure_reason: Option<FailureReason>,
        error_detail: Option<ErrorDetail>,
    ) -> Self {
        Self {
            path,
            passed,
            failure_reason,
            error_detail,
        }
    }

    /// Format a short recent-failure entry for the dashboard footer.
    pub fn failure_message(&self) -> Option<String> {
        let error_detail = self.error_detail.as_ref()?;
        let filename = match self.path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_string(),
            None => "<invalid-filename>".to_string(),
        };
        Some(format!("{}: {}", filename, error_detail.message))
    }
}

/// Shared inputs and control flags consumed by the worker thread.
pub struct WorkerLoopContext {
    /// Manifest coordinator used to apply and persist corpus-level results.
    pub manifest_store: DashboardManifest,
    /// Cache used for roundtrip results.
    pub cache: UnifiedCache,
    /// Remaining corpora to process, with display and size metadata.
    pub corpus_paths: Vec<(String, String, usize, usize)>,
    /// Channel used to stream dashboard state updates to the UI thread.
    pub event_tx: Sender<DashboardEvent>,
    /// Stop flag toggled by the UI.
    pub should_stop: Arc<AtomicBool>,
    /// Pause flag toggled between corpora or by the UI.
    pub should_pause: Arc<AtomicBool>,
    /// Skip flag used to abandon the current corpus.
    pub should_skip_corpus: Arc<AtomicBool>,
    /// Whether to continue automatically between corpora.
    pub auto_mode: bool,
}

/// One file-level progress update emitted by the worker thread.
#[derive(Clone, Debug)]
pub struct FileProgressUpdate {
    /// One-based file index within the current corpus.
    pub tested: usize,
    /// Whether the file result came from cache.
    pub cache_hit: bool,
    /// Whether the file passed.
    pub passed: bool,
    /// Optional failure message for the recent-failures panel.
    pub failure_message: Option<String>,
}

/// Reducer events sent from the worker thread to the UI thread.
#[derive(Clone, Debug)]
pub enum DashboardEvent {
    /// A new corpus has started processing.
    CorpusStarted {
        /// Zero-based corpus index.
        corpus_idx: usize,
        /// Human-readable corpus name.
        corpus_name: String,
        /// Total files in the corpus.
        file_count: usize,
    },
    /// One file result has been observed.
    FileProgress(FileProgressUpdate),
    /// Aggregate totals for the just-completed corpus have been committed.
    TotalsCommitted {
        /// Display name of the completed corpus.
        corpus_name: String,
        /// Number of newly passed files in the corpus.
        newly_passed: usize,
        /// Number of newly failed files in the corpus.
        newly_failed: usize,
    },
    /// Free-form status update from the worker thread.
    Status {
        /// Whether the worker is actively testing.
        is_testing: bool,
        /// Whether the worker expects the UI to remain paused.
        is_paused: bool,
        /// Status line shown in the footer.
        message: String,
    },
    /// The worker thread finished its run.
    Finished,
}

/// CLI switches that control cache usage and corpus-to-corpus pausing.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Disable caching of test results.
    #[arg(long)]
    pub no_cache: bool,
    /// Auto-run without pausing between corpora.
    #[arg(long)]
    pub auto: bool,
}

/// Mutable dashboard model rendered by the UI thread and updated by the worker thread.
#[derive(Clone)]
pub struct AppState {
    /// Zero-based index of the currently active corpus.
    pub current_corpus_idx: usize,
    /// Human-readable corpus name.
    pub current_corpus_name: String,
    /// Number of files processed within the current corpus.
    pub current_corpus_files_tested: usize,
    /// Total file count in the current corpus.
    pub current_corpus_total_files: usize,
    /// Current-corpus pass count.
    pub current_corpus_passed: usize,
    /// Current-corpus failure count.
    pub current_corpus_failed: usize,
    /// Total corpus count in the manifest.
    pub total_corpora: usize,
    /// Total manifest file count.
    pub total_files: usize,
    /// Global pass count.
    pub total_passed: usize,
    /// Global failure count.
    pub total_failed: usize,
    /// Remaining untested file count.
    pub total_not_tested: usize,
    /// Cache hit count.
    pub cache_hits: usize,
    /// Cache miss count.
    pub cache_misses: usize,
    /// Whether the UI is currently paused.
    pub is_paused: bool,
    /// Whether the worker is actively testing a corpus.
    pub is_testing: bool,
    /// Free-form status message shown in the footer.
    pub status_message: String,
    /// Rolling list of recent failures.
    pub recent_failures: Vec<String>,
    /// Dashboard start time.
    pub start_time: Instant,
    /// Elapsed wall-clock seconds.
    pub elapsed_seconds: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Initialize empty runtime state before manifest values are copied in.
    pub fn new() -> Self {
        Self {
            current_corpus_idx: 0,
            current_corpus_name: String::new(),
            current_corpus_files_tested: 0,
            current_corpus_total_files: 0,
            current_corpus_passed: 0,
            current_corpus_failed: 0,
            total_corpora: 0,
            total_files: 0,
            total_passed: 0,
            total_failed: 0,
            total_not_tested: 0,
            cache_hits: 0,
            cache_misses: 0,
            is_paused: false,
            is_testing: false,
            status_message: "Starting...".to_string(),
            recent_failures: Vec::new(),
            start_time: Instant::now(),
            elapsed_seconds: 0,
        }
    }

    /// Seed summary totals from the corpus manifest.
    pub fn initialize_from_manifest(&mut self, manifest: &CorpusManifest) {
        self.total_corpora = manifest.total_corpora;
        self.total_files = manifest.total_files;
        self.total_passed = manifest.total_passed;
        self.total_failed = manifest.total_failed;
        self.total_not_tested = manifest.total_not_tested;
        self.status_message = format!(
            "Loaded {} corpora, {} files total",
            manifest.total_corpora, manifest.total_files
        );
    }

    /// Apply one worker-thread event to the local UI-owned state.
    pub fn apply_event(&mut self, event: DashboardEvent) {
        match event {
            DashboardEvent::CorpusStarted {
                corpus_idx,
                corpus_name,
                file_count,
            } => {
                self.current_corpus_idx = corpus_idx;
                self.current_corpus_name = corpus_name.clone();
                self.current_corpus_total_files = file_count;
                self.current_corpus_files_tested = 0;
                self.current_corpus_passed = 0;
                self.current_corpus_failed = 0;
                self.is_testing = true;
                self.is_paused = false;
                self.status_message = format!("Testing corpus: {}", corpus_name);
            }
            DashboardEvent::FileProgress(update) => {
                self.current_corpus_files_tested = update.tested;
                if update.cache_hit {
                    self.cache_hits += 1;
                } else {
                    self.cache_misses += 1;
                }

                if update.passed {
                    self.current_corpus_passed += 1;
                } else {
                    self.current_corpus_failed += 1;
                    if let Some(message) = update.failure_message {
                        self.push_recent_failure(message);
                    }
                }
            }
            DashboardEvent::TotalsCommitted {
                corpus_name,
                newly_passed,
                newly_failed,
            } => {
                self.total_passed += newly_passed;
                self.total_failed += newly_failed;
                self.total_not_tested = self
                    .total_not_tested
                    .saturating_sub(newly_passed + newly_failed);
                self.is_testing = false;
                self.status_message = format!("Completed corpus: {}", corpus_name);
            }
            DashboardEvent::Status {
                is_testing,
                is_paused,
                message,
            } => {
                self.is_testing = is_testing;
                self.is_paused = is_paused;
                self.status_message = message;
            }
            DashboardEvent::Finished => {
                self.is_testing = false;
                self.status_message = "All corpora tested!".to_string();
            }
        }
    }

    /// Refresh elapsed time from the original dashboard start instant.
    pub fn tick_elapsed(&mut self) {
        self.elapsed_seconds = self.start_time.elapsed().as_secs();
    }

    /// Append one recent failure while preserving the rolling window size.
    fn push_recent_failure(&mut self, message: String) {
        self.recent_failures.push(message);
        if self.recent_failures.len() > 10 {
            self.recent_failures.remove(0);
        }
    }

    /// Percentage of tested files answered from cache.
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / total as f64) * 100.0
        }
    }

    /// Percentage of manifest files that have reached pass/fail status.
    pub fn overall_progress_pct(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            ((self.total_passed + self.total_failed) as f64 / self.total_files as f64) * 100.0
        }
    }

    /// Percentage complete for the currently active corpus.
    pub fn current_corpus_progress_pct(&self) -> f64 {
        if self.current_corpus_total_files == 0 {
            0.0
        } else {
            (self.current_corpus_files_tested as f64 / self.current_corpus_total_files as f64)
                * 100.0
        }
    }

    /// End-to-end throughput from dashboard start time.
    pub fn files_per_second(&self) -> f64 {
        if self.elapsed_seconds == 0 {
            0.0
        } else {
            (self.total_passed + self.total_failed) as f64 / self.elapsed_seconds as f64
        }
    }

    /// Estimated remaining wall-clock seconds, if throughput is non-zero.
    pub fn eta_seconds(&self) -> Option<u64> {
        let files_per_sec = self.files_per_second();
        if files_per_sec > 0.0 && self.total_not_tested > 0 {
            Some((self.total_not_tested as f64 / files_per_sec) as u64)
        } else {
            None
        }
    }
}

/// Enter alternate-screen raw mode and construct the TUI terminal.
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, Box<dyn std::error::Error>>
{
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

/// Restore terminal modes so crashes or early exits do not leave the shell in raw mode.
pub fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Resolve the user home directory for manifest and cache paths, exiting if unavailable.
pub fn home_dir_or_exit() -> PathBuf {
    match dirs::home_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Failed to get home directory");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use talkbank_transform::corpus::manifest::{ErrorDetail, FailureReason};

    use super::{AppState, DashboardEvent, FileProgressUpdate, FileTestOutcome};

    #[test]
    fn apply_event_updates_current_corpus_progress() {
        let mut state = AppState::new();
        state.apply_event(DashboardEvent::CorpusStarted {
            corpus_idx: 2,
            corpus_name: "Demo".to_string(),
            file_count: 4,
        });
        state.apply_event(DashboardEvent::FileProgress(FileProgressUpdate {
            tested: 1,
            cache_hit: true,
            passed: false,
            failure_message: Some("sample.cha: mismatch".to_string()),
        }));

        assert_eq!(state.current_corpus_idx, 2);
        assert_eq!(state.current_corpus_name, "Demo");
        assert_eq!(state.current_corpus_total_files, 4);
        assert_eq!(state.current_corpus_files_tested, 1);
        assert_eq!(state.current_corpus_failed, 1);
        assert_eq!(state.cache_hits, 1);
        assert_eq!(state.recent_failures, vec!["sample.cha: mismatch"]);
    }

    #[test]
    fn apply_event_commits_totals() {
        let mut state = AppState::new();
        state.total_not_tested = 5;

        state.apply_event(DashboardEvent::TotalsCommitted {
            corpus_name: "Corpus A".to_string(),
            newly_passed: 3,
            newly_failed: 1,
        });

        assert_eq!(state.total_passed, 3);
        assert_eq!(state.total_failed, 1);
        assert_eq!(state.total_not_tested, 1);
        assert_eq!(state.status_message, "Completed corpus: Corpus A");
        assert!(!state.is_testing);
    }

    #[test]
    fn file_test_outcome_formats_failure_message_from_filename() {
        let outcome = FileTestOutcome::new(
            PathBuf::from("/tmp/demo/sample.cha"),
            false,
            Some(FailureReason::ParseError),
            Some(ErrorDetail::new("ParseError", "expected tier marker")),
        );

        assert_eq!(
            outcome.failure_message().as_deref(),
            Some("sample.cha: expected tier marker")
        );
    }
}
