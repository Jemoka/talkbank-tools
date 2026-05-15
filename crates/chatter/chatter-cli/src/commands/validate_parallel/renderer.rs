//! Output renderers for non-interactive parallel validation.
//!
//! The runtime drives a single event stream, while concrete renderers decide
//! how to present those events as text or JSONL.

use std::path::Path;

use indicatif::ProgressStyle;

use crate::output::{CASCADING_HINT, print_errors, should_show_cascading_hint};
use crate::progress::ProgressThrottle;
use talkbank_transform::validation_runner::{
    ErrorEvent, FileCompleteEvent, FileStatus, RoundtripEvent, ValidationStatsSnapshot,
};

/// Build the non-interactive renderer appropriate for the requested output mode.
pub fn create_renderer(json_mode: bool, quiet: bool) -> Box<dyn ValidationRenderer> {
    if json_mode {
        Box::new(JsonRenderer)
    } else {
        Box::new(TextRenderer::new(quiet))
    }
}

/// Rendering interface for streamed validation events.
pub trait ValidationRenderer {
    /// Handle the start of file discovery.
    fn handle_discovering(&mut self);
    /// Handle the start of validation once the total file count is known.
    fn handle_started(&mut self, total_files: usize);
    /// Render one batch of file errors and return the number counted toward `--max-errors`.
    fn handle_errors(&mut self, error_event: &ErrorEvent) -> usize;
    /// Render one roundtrip result and return the number counted toward `--max-errors`.
    fn handle_roundtrip_complete(&mut self, event: &RoundtripEvent) -> usize;
    /// Render one completed file.
    fn handle_file_complete(&mut self, file_event: &FileCompleteEvent, files_completed: usize);
    /// Finalize any progress display at the end of the run.
    fn handle_finished(
        &mut self,
        stats: &ValidationStatsSnapshot,
        files_completed: usize,
        max_errors: Option<usize>,
        error_count: usize,
    );
    /// Emit the final run summary.
    fn print_summary(&self, path: &Path, stats: &ValidationStatsSnapshot, roundtrip: bool);
}

/// Human-readable renderer with optional progress throttling.
struct TextRenderer {
    /// Whether normal success output should be suppressed.
    quiet: bool,
    /// Progress bar active until the first detailed error output.
    progress_bar: Option<ProgressThrottle>,
    /// Whether valid-file output should be suppressed after rich error rendering begins.
    suppress_valid_output: bool,
}

impl TextRenderer {
    /// Create a new text renderer.
    fn new(quiet: bool) -> Self {
        Self {
            quiet,
            progress_bar: if quiet {
                None
            } else {
                Some(ProgressThrottle::new(
                    0,
                    "{spinner:.green} [{bar:40.cyan/blue}] {msg} {elapsed_precise}",
                    "#>-",
                    1,
                    200,
                ))
            },
            suppress_valid_output: false,
        }
    }

    /// Clear the progress bar before detailed error output.
    fn suspend_progress_for_detail(&mut self) {
        if let Some(mut progress_bar) = self.progress_bar.take() {
            progress_bar.finish_and_clear();
            self.suppress_valid_output = true;
        }
    }
}

impl ValidationRenderer for TextRenderer {
    fn handle_discovering(&mut self) {
        if let Some(ref progress_bar) = self.progress_bar {
            progress_bar.set_length(1);
            progress_bar.bar().set_message("Discovering files...");
        }
    }

    fn handle_started(&mut self, total_files: usize) {
        if let Some(ref progress_bar) = self.progress_bar {
            progress_bar.set_length(total_files as u64);
            let style = match ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {msg} {elapsed_precise}")
            {
                Ok(style) => style,
                Err(error) => {
                    eprintln!("Error setting progress style: {}", error);
                    ProgressStyle::default_bar()
                }
            };
            progress_bar.bar().set_style(style.progress_chars("#>-"));
        }
    }

    fn handle_errors(&mut self, error_event: &ErrorEvent) -> usize {
        self.suspend_progress_for_detail();
        print_errors(&error_event.path, &error_event.source, &error_event.errors);
        if error_event.source.is_empty() {
            eprintln!("  note: errors from cache (use --force to re-validate)");
        }
        if !self.quiet && should_show_cascading_hint(&error_event.errors) {
            eprintln!("{}", CASCADING_HINT);
        }
        error_event.errors.len()
    }

    fn handle_roundtrip_complete(&mut self, event: &RoundtripEvent) -> usize {
        if event.passed {
            return 0;
        }

        self.suspend_progress_for_detail();
        eprintln!("✗ {} (roundtrip failed)", event.path.display());
        if let Some(ref reason) = event.failure_reason {
            eprintln!("  {}", reason);
        }
        if let Some(ref diff) = event.diff {
            eprintln!("{}", diff);
        }
        1
    }

    fn handle_file_complete(&mut self, file_event: &FileCompleteEvent, files_completed: usize) {
        let cache_hit = status_is_cache_hit(&file_event.status);

        if let Some(ref mut progress_bar) = self.progress_bar {
            let is_error = matches!(
                file_event.status,
                FileStatus::Invalid { .. }
                    | FileStatus::RoundtripFailed { .. }
                    | FileStatus::ParseError { .. }
                    | FileStatus::ReadError { .. }
            );
            if is_error {
                progress_bar.bar().set_position(files_completed as u64);
                let filename = file_event
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_event.path.display().to_string());
                let message = match &file_event.status {
                    FileStatus::Valid { .. } => format!("✓ {}", filename),
                    _ => format!("✗ {}", filename),
                };
                progress_bar.set_message_throttled(message, true);
            }
        }

        match &file_event.status {
            FileStatus::Valid { .. } => {
                if self.progress_bar.is_none() && !self.quiet && !self.suppress_valid_output {
                    let cache_suffix = if cache_hit { " (cached)" } else { "" };
                    println!("✓ {}{}", file_event.path.display(), cache_suffix);
                }
            }
            FileStatus::Invalid { .. } => {}
            FileStatus::RoundtripFailed { .. } => {}
            FileStatus::ParseError { message } => {
                eprintln!("✗ {} (parse error: {})", file_event.path.display(), message);
            }
            FileStatus::ReadError { message } => {
                eprintln!("✗ {} (read error: {})", file_event.path.display(), message);
            }
        }
    }

    fn handle_finished(
        &mut self,
        stats: &ValidationStatsSnapshot,
        files_completed: usize,
        max_errors: Option<usize>,
        error_count: usize,
    ) {
        if let Some(ref mut progress_bar) = self.progress_bar {
            progress_bar.bar().set_position(files_completed as u64);
            if stats.cancelled {
                progress_bar
                    .bar()
                    .finish_with_message("Validation cancelled");
            } else {
                progress_bar
                    .bar()
                    .finish_with_message("Validation complete");
            }
        }

        if let Some(limit) = max_errors
            && error_count >= limit
        {
            eprintln!("Stopped after reaching --max-errors {}", limit);
        }
    }

    fn print_summary(&self, _path: &Path, stats: &ValidationStatsSnapshot, roundtrip: bool) {
        if self.quiet {
            return;
        }

        println!("\n=== Summary ===");
        println!("Total files: {}", stats.total_files);
        println!("Valid: {}", stats.valid_files);
        println!("Invalid: {}", stats.invalid_files);
        if stats.parse_errors > 0 {
            println!("Parse errors: {}", stats.parse_errors);
        }
        if roundtrip {
            println!("\n=== Roundtrip ===");
            println!("Passed: {}", stats.roundtrip_passed);
            println!("Failed: {}", stats.roundtrip_failed);
        }
        if stats.cancelled {
            println!("Status: CANCELLED");
        }
        println!("\n=== Cache Statistics ===");
        println!("Cache hits: {}", stats.cache_hits);
        println!("Cache misses: {}", stats.cache_misses);
        println!("Hit rate: {:.1}%", stats.cache_hit_rate());
    }
}

/// JSONL renderer for machine-readable output.
struct JsonRenderer;

impl ValidationRenderer for JsonRenderer {
    fn handle_discovering(&mut self) {}

    fn handle_started(&mut self, _total_files: usize) {}

    fn handle_errors(&mut self, error_event: &ErrorEvent) -> usize {
        let json_errors: Vec<_> = error_event
            .errors
            .iter()
            .map(|error| {
                serde_json::json!({
                    "code": error.code.to_string(),
                    "severity": format!("{:?}", error.severity),
                    "message": error.message
                })
            })
            .collect();

        let mut line = serde_json::json!({
            "type": "file",
            "file": error_event.path.to_string_lossy(),
            "status": "invalid",
            "error_count": error_event.errors.len(),
            "errors": json_errors
        });

        if should_show_cascading_hint(&error_event.errors) {
            line["note"] = serde_json::json!(
                "Some additional checks may not have run because of structural errors. Fix the structural errors first, then re-validate."
            );
        }

        println!("{}", line);
        error_event.errors.len()
    }

    fn handle_roundtrip_complete(&mut self, event: &RoundtripEvent) -> usize {
        if event.passed {
            return 0;
        }

        let line = serde_json::json!({
            "type": "file",
            "file": event.path.to_string_lossy(),
            "status": "roundtrip_failed",
            "reason": event.failure_reason,
            "diff": event.diff
        });
        println!("{}", line);
        1
    }

    fn handle_file_complete(&mut self, file_event: &FileCompleteEvent, _files_completed: usize) {
        let cache_hit = status_is_cache_hit(&file_event.status);

        match &file_event.status {
            FileStatus::Valid { .. } => {
                let line = serde_json::json!({
                    "type": "file",
                    "file": file_event.path.to_string_lossy(),
                    "status": "valid",
                    "cache_hit": cache_hit
                });
                println!("{}", line);
            }
            FileStatus::Invalid { .. } => {}
            FileStatus::RoundtripFailed { .. } => {}
            FileStatus::ParseError { message } => {
                let line = serde_json::json!({
                    "type": "file",
                    "file": file_event.path.to_string_lossy(),
                    "status": "parse_error",
                    "error": message
                });
                println!("{}", line);
            }
            FileStatus::ReadError { message } => {
                let line = serde_json::json!({
                    "type": "file",
                    "file": file_event.path.to_string_lossy(),
                    "status": "read_error",
                    "error": message
                });
                println!("{}", line);
            }
        }
    }

    fn handle_finished(
        &mut self,
        _stats: &ValidationStatsSnapshot,
        _files_completed: usize,
        max_errors: Option<usize>,
        error_count: usize,
    ) {
        if let Some(limit) = max_errors
            && error_count >= limit
        {
            eprintln!("Stopped after reaching --max-errors {}", limit);
        }
    }

    fn print_summary(&self, path: &Path, stats: &ValidationStatsSnapshot, roundtrip: bool) {
        let mut summary = serde_json::json!({
            "type": "summary",
            "directory": path.to_string_lossy(),
            "total_files": stats.total_files,
            "valid": stats.valid_files,
            "invalid": stats.invalid_files,
            "parse_errors": stats.parse_errors,
            "cache_hits": stats.cache_hits,
            "cache_misses": stats.cache_misses,
            "cache_hit_rate": stats.cache_hit_rate(),
            "cancelled": stats.cancelled
        });
        if roundtrip {
            summary["roundtrip_passed"] = serde_json::json!(stats.roundtrip_passed);
            summary["roundtrip_failed"] = serde_json::json!(stats.roundtrip_failed);
        }
        println!("{}", summary);
    }
}

/// Return whether this file status represents a cache hit.
fn status_is_cache_hit(status: &FileStatus) -> bool {
    matches!(
        status,
        FileStatus::Valid { cache_hit: true }
            | FileStatus::Invalid {
                cache_hit: true,
                ..
            }
            | FileStatus::RoundtripFailed {
                cache_hit: true,
                ..
            }
    )
}
