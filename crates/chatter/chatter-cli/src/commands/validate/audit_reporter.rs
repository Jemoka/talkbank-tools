//! Streaming audit reporting for bulk validation runs.
//!
//! Audit mode needs to write JSONL diagnostics for large corpora without holding
//! all errors in memory. This module keeps that streaming behavior, but it no
//! longer shares a `BufWriter<File>` and stats accumulator behind mutexes.
//! Instead, [`AuditReporter`] owns a dedicated writer thread and exposes a
//! cloneable [`AuditReporterHandle`] for worker threads to send file results to.
//!
//! That split makes the concurrency boundary explicit:
//! - validation workers only send immutable audit events
//! - the writer thread owns file IO and summary accounting
//! - shutdown is a single `finish()` call that joins the writer thread and
//!   returns the final [`AuditStats`]

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use talkbank_model::{ErrorCode, ParseError};

/// Number of in-flight audit messages allowed before worker threads block.
const AUDIT_CHANNEL_CAPACITY: usize = 256;

/// Statistics collected during audit runs to support the CLI’s bulk validation mode.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AuditStats {
    /// Total files processed.
    pub total_files: usize,
    /// Files with one or more errors.
    pub files_with_errors: usize,
    /// Total error count across all files.
    pub total_errors: usize,
    /// Error totals grouped by code.
    pub errors_by_code: HashMap<ErrorCode, usize>,
    /// Distinct file paths grouped by error code.
    pub files_by_code: HashMap<ErrorCode, HashSet<String>>,
}

impl AuditStats {
    /// Start a fresh audit accumulator with zeroed counters and empty maps.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an error occurrence for one file and one diagnostic code.
    pub fn record_error(&mut self, file: &str, code: ErrorCode) {
        self.total_errors += 1;
        *self.errors_by_code.entry(code).or_insert(0) += 1;
        self.files_by_code
            .entry(code)
            .or_default()
            .insert(file.to_string());
    }

    /// Mark a file as processed, tracking whether it produced any errors.
    pub fn mark_file_processed(&mut self, had_errors: bool) {
        self.total_files += 1;
        if had_errors {
            self.files_with_errors += 1;
        }
    }

    /// Print the audit summary shown after CLI audit runs complete.
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(80));
        println!("VALIDATION AUDIT SUMMARY");
        println!("{}", "=".repeat(80));
        println!();
        println!("Total files processed: {}", self.total_files);
        println!(
            "Files with errors: {} ({:.1}%)",
            self.files_with_errors,
            if self.total_files > 0 {
                100.0 * self.files_with_errors as f64 / self.total_files as f64
            } else {
                0.0
            }
        );
        println!("Total errors: {}", self.total_errors);
        println!();

        if !self.errors_by_code.is_empty() {
            println!("Errors by code:");
            let mut codes: Vec<_> = self.errors_by_code.iter().collect();
            codes.sort_by_key(|(code, _)| code.as_str());
            for (code, count) in codes {
                let file_count = self
                    .files_by_code
                    .get(code)
                    .map(|files| files.len())
                    .unwrap_or(0);
                println!(
                    "  {}: {} errors in {} files",
                    code.as_str(),
                    count,
                    file_count
                );
            }
        }
        println!();
    }
}

/// Cloneable worker-side handle used to report completed file results.
#[derive(Clone)]
pub struct AuditReporterHandle {
    sender: Sender<AuditCommand>,
}

impl AuditReporterHandle {
    /// Create a new reporting handle for the given audit command sender.
    fn new(sender: Sender<AuditCommand>) -> Self {
        Self { sender }
    }

    /// Report all parse or validation errors for a file and mark it as processed.
    pub fn report_file_results(&self, file_path: &str, errors: Vec<ParseError>) {
        self.send(AuditCommand::FileResults {
            file_path: file_path.to_string(),
            errors,
        });
    }

    /// Mark a file as processed when no structured error list is available.
    pub fn mark_file_done(&self, had_errors: bool) {
        self.send(AuditCommand::FileDone { had_errors });
    }

    /// Send one audit command to the writer thread.
    fn send(&self, command: AuditCommand) {
        if let Err(error) = self.sender.send(command) {
            eprintln!(
                "Warning: Failed to send audit event to writer thread: {}",
                error
            );
        }
    }
}

/// Lifecycle owner for the dedicated audit writer thread.
pub struct AuditReporter {
    reporter: Option<AuditReporterHandle>,
    worker: Option<JoinHandle<std::io::Result<AuditStats>>>,
}

impl AuditReporter {
    /// Create a new audit reporter that writes JSONL records to `output_path`.
    pub fn new(output_path: &Path) -> std::io::Result<Self> {
        let file = File::create(output_path)?;
        let writer = BufWriter::new(file);
        let (sender, receiver) = crossbeam_channel::bounded(AUDIT_CHANNEL_CAPACITY);
        let worker = std::thread::spawn(move || run_audit_writer(writer, receiver));

        Ok(Self {
            reporter: Some(AuditReporterHandle::new(sender)),
            worker: Some(worker),
        })
    }

    /// Return a cloneable reporting handle for worker threads.
    pub fn reporter(&self) -> AuditReporterHandle {
        // Caller-contract invariant: `reporter()` must be called
        // before `finish()` consumes the AuditReporterTask. The
        // `self.reporter` Option is `Some(...)` from `try_new` until
        // `shutdown_and_join()` takes it.
        #[allow(clippy::expect_used)]
        self.reporter
            .as_ref()
            .expect("audit reporter requested after shutdown")
            .clone()
    }

    /// Finish the audit run, flush output, and return final summary statistics.
    pub fn finish(mut self) -> std::io::Result<AuditStats> {
        self.shutdown_and_join()
    }

    /// Shut down the writer thread and join it if it is still running.
    fn shutdown_and_join(&mut self) -> std::io::Result<AuditStats> {
        if let Some(reporter) = self.reporter.take() {
            reporter.send(AuditCommand::Shutdown);
        }
        let Some(worker) = self.worker.take() else {
            return Ok(AuditStats::new());
        };

        worker
            .join()
            .map_err(|_| std::io::Error::other("audit writer thread panicked"))?
    }
}

impl Drop for AuditReporter {
    /// Join the writer thread on drop so buffered audit output is not lost.
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_and_join() {
            eprintln!("Warning: Failed to finalize audit output: {}", error);
        }
    }
}

/// Commands sent from validation workers to the audit writer thread.
enum AuditCommand {
    /// Report the full structured results for one completed file.
    FileResults {
        /// File path used in JSONL output and summary accounting.
        file_path: String,
        /// Errors emitted while processing the file.
        errors: Vec<ParseError>,
    },
    /// Mark a file complete when no structured results can be reported.
    FileDone {
        /// Whether the file should count as erroneous in the summary.
        had_errors: bool,
    },
    /// Stop the writer thread after all earlier commands have been processed.
    Shutdown,
}

/// Run the dedicated audit writer loop until it receives a shutdown command.
fn run_audit_writer(
    mut writer: BufWriter<File>,
    receiver: Receiver<AuditCommand>,
) -> std::io::Result<AuditStats> {
    let mut stats = AuditStats::new();

    for command in receiver {
        match command {
            AuditCommand::FileResults { file_path, errors } => {
                let had_errors = !errors.is_empty();
                for error in errors {
                    stats.record_error(&file_path, error.code);
                    write_error_record(&mut writer, &file_path, &error);
                }
                stats.mark_file_processed(had_errors);
            }
            AuditCommand::FileDone { had_errors } => {
                stats.mark_file_processed(had_errors);
            }
            AuditCommand::Shutdown => {
                break;
            }
        }
    }

    writer.flush()?;
    Ok(stats)
}

/// Write one JSONL audit record for a single parse or validation error.
fn write_error_record(writer: &mut BufWriter<File>, file_path: &str, error: &ParseError) {
    let json = serde_json::json!({
        "file": file_path,
        "code": error.code.as_str(),
        "message": error.message,
        "line": error.location.line,
        "column": error.location.column,
    });

    if let Err(write_error) = writeln!(writer, "{}", json) {
        eprintln!(
            "Warning: Failed to write error to audit file for {}: {}",
            file_path, write_error
        );
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the audit reporter boundary.

    use std::collections::{HashMap, HashSet};
    use std::fs;

    use serde_json::Value;
    use talkbank_model::{ErrorCode, ParseError, Severity, SourceLocation};

    use super::{AuditReporter, AuditStats};

    /// Sequential reports should produce matching JSONL output and summary stats.
    #[test]
    fn finish_returns_stats_and_jsonl_output() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let output_path = temp_dir.path().join("audit.jsonl");
        let reporter = AuditReporter::new(&output_path).expect("audit reporter should be created");
        let handle = reporter.reporter();

        handle.report_file_results("one.cha", vec![test_error("first error", 2, 4)]);
        handle.report_file_results(
            "two.cha",
            vec![
                test_error("second error", 5, 1),
                test_error("third error", 8, 3),
            ],
        );
        handle.mark_file_done(false);

        let stats = reporter
            .finish()
            .expect("audit reporter should finish cleanly");
        let output = fs::read_to_string(&output_path).expect("audit output should be readable");
        let lines: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).expect("line should be valid JSON"))
            .collect();

        assert_eq!(
            stats,
            AuditStats {
                total_files: 3,
                files_with_errors: 2,
                total_errors: 3,
                errors_by_code: HashMap::from([(ErrorCode::TestError, 3)]),
                files_by_code: HashMap::from([(
                    ErrorCode::TestError,
                    HashSet::from([String::from("one.cha"), String::from("two.cha")]),
                )]),
            }
        );
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["file"], "one.cha");
        assert_eq!(lines[1]["message"], "second error");
        assert_eq!(lines[2]["line"], 8);
        assert_eq!(lines[2]["column"], 3);
    }

    /// Finishing an unused reporter should still flush a valid empty file and zero stats.
    #[test]
    fn finish_without_reports_returns_empty_stats() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let output_path = temp_dir.path().join("audit.jsonl");
        let reporter = AuditReporter::new(&output_path).expect("audit reporter should be created");

        let stats = reporter
            .finish()
            .expect("audit reporter should finish cleanly");
        let output = fs::read_to_string(&output_path).expect("audit output should be readable");

        assert_eq!(stats, AuditStats::new());
        assert!(output.is_empty());
    }

    /// Build a deterministic test error with line and column information.
    fn test_error(message: &str, line: usize, column: usize) -> ParseError {
        ParseError::new(
            ErrorCode::TestError,
            Severity::Error,
            SourceLocation::from_offsets_with_position(0, 1, line, column),
            Option::<talkbank_model::ErrorContext>::None,
            message,
        )
    }
}
