//! Terminal progress display for batch job polling.
//!
//! While the CLI polls a running server job, the user needs visual feedback
//! showing how many files have completed, which file is currently being
//! processed, and roughly how long the job has been running.
//!
//! This module provides [`BatchProgress`], an indicatif-based implementation
//! that renders a two-line display: a determinate progress bar for overall
//! file completion and a spinner showing the active processing stage. Both
//! implement the [`ProgressSink`] trait so the polling loop can be decoupled
//! from the rendering backend. The ratatui TUI uses a separate
//! reducer-message sender that implements the same trait while the render loop
//! owns UI state locally.

use crate::api::{FileStatusEntry, FileStatusKind, HealthResponse};
use indicatif::{ProgressBar, ProgressStyle};

/// Trait for receiving progress updates during job polling.
///
/// Implemented by `BatchProgress` (indicatif bars) and `TuiProgress`
/// (reducer-message sender for the ratatui runtime).
pub trait ProgressSink: Send + Sync {
    /// Update completed file count and file status entries.
    fn update(&self, done: u64, file_statuses: &[FileStatusEntry]);
    /// Log a successfully completed file.
    fn log_done(&self, filename: &str);
    /// Log a failed file with error message.
    fn log_error(&self, filename: &str, msg: &str);
    /// Mark processing as complete.
    fn finish(&self);
    /// Update server health snapshot. Default no-op for non-TUI sinks.
    fn update_health(&self, _health: &HealthResponse) {}
    /// Update batch-level language-group progress. Default no-op.
    fn update_batch_progress(&self, _progress: &crate::api::BatchInferProgress) {}
    /// Surface a cancellation receipt for the end-of-run banner.
    /// Default no-op for non-TUI sinks (which already print the
    /// receipt to stderr inline as part of their finish() output).
    fn send_cancelled_receipt(&self, _receipt: crate::cli::tui::app::CancelledReceipt) {}
}

/// Progress display for batch processing — overall bar + activity spinner.
///
/// Shows:
/// ```text
///   [=====>                  ] 3/50 files  [00:42]
///   ⠋ morphotag: stanza processing
/// ```
pub struct BatchProgress {
    mp: indicatif::MultiProgress,
    overall: ProgressBar,
    activity: ProgressBar,
    command: String,
}

impl BatchProgress {
    /// Create a new batch progress display.
    pub fn new(total: u64, command: &str) -> Self {
        let mp = indicatif::MultiProgress::new();

        let overall = mp.add(ProgressBar::new(total));
        // indicatif template strings are validated at compile time
        // by the `template(...)` parser; the literal here is fixed.
        #[allow(clippy::expect_used)]
        overall.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:30.cyan/dim}] {pos}/{len} files  [{elapsed_precise}]")
                .expect("valid template")
                .progress_chars("=>-"),
        );
        overall.set_position(0);

        let activity = mp.add(ProgressBar::new_spinner());
        // Same template-literal invariant.
        #[allow(clippy::expect_used)]
        activity.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.blue} {msg}")
                .expect("valid template"),
        );
        activity.enable_steady_tick(std::time::Duration::from_millis(120));

        Self {
            mp,
            overall,
            activity,
            command: command.to_string(),
        }
    }

    /// Update completed file count and activity from file status entries.
    pub fn update(&self, done: u64, file_statuses: &[FileStatusEntry]) {
        self.overall.set_position(done);

        // Find a file with an active progress label to show activity
        if let Some(active) = file_statuses
            .iter()
            .find(|f| f.progress_label.is_some() && f.status == FileStatusKind::Processing)
        {
            let label = active.progress_label.as_deref().unwrap_or("processing");
            let pct = match (active.progress_current, active.progress_total) {
                (Some(c), Some(t)) if t > 0 => format!(" ({c}/{t})"),
                _ => String::new(),
            };
            self.activity
                .set_message(format!("{}: {label}{pct}", self.command));
        }
    }

    /// Log a successfully completed file (printed above the progress bar).
    pub fn log_done(&self, filename: &str) {
        self.overall.println(format!("  \u{2713} {filename}"));
    }

    /// Log a failed file (printed above the progress bar).
    pub fn log_error(&self, filename: &str, msg: &str) {
        let first_line = msg.split('\n').next().unwrap_or("unknown error");
        self.overall
            .println(format!("  \u{2717} {filename}: {first_line}"));
    }

    /// Mark processing as complete and clear the bars.
    pub fn finish(&self) {
        self.activity.finish_and_clear();
        self.overall.finish_and_clear();
        // Force clear the multi-progress to avoid ghost lines
        let _ = &self.mp;
    }
}

impl ProgressSink for BatchProgress {
    fn update(&self, done: u64, file_statuses: &[FileStatusEntry]) {
        self.update(done, file_statuses);
    }

    fn log_done(&self, filename: &str) {
        self.log_done(filename);
    }

    fn log_error(&self, filename: &str, msg: &str) {
        self.log_error(filename, msg);
    }

    fn finish(&self) {
        self.finish();
    }

    fn update_batch_progress(&self, progress: &crate::api::BatchInferProgress) {
        self.update_batch_progress(progress);
    }
}

// -- Batch progress extension --
impl BatchProgress {
    /// Update the activity spinner with batch-level language-group progress.
    ///
    /// Called when the polled job response includes `batch_progress` from a
    /// running batched text command. Shows a summary like:
    /// `morphotag: 2/4 languages done, 1200/1800 utterances (67%)`
    pub fn update_batch_progress(&self, progress: &crate::api::BatchInferProgress) {
        self.activity
            .set_message(format!("{}: {}", self.command, progress.summary()));
    }
}
