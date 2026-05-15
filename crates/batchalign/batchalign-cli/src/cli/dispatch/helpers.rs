//! Shared helper functions for dispatch modes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ReleasedCommand;
use crate::api::{
    DisplayPath, FilePayload, FileStatusEntry, FileStatusKind, JobId, JobInfo, JobStatus,
};
use crate::debug_artifacts::JobDebugArtifacts;

use crate::cli::client::{BatchalignClient, MAX_POLL_FAILURES, POLL_MAX, POLL_MIN, POLL_STEP};
use crate::cli::error::CliError;
use crate::cli::output;
use crate::cli::progress::ProgressSink;

// ---------------------------------------------------------------------------
// Single-server incremental poll
// ---------------------------------------------------------------------------

/// Named CLI-visible file failure detail.
#[derive(Debug, Clone)]
pub(super) struct FileErrorDetail {
    /// File identity reported to the user.
    pub filename: DisplayPath,
    /// Human-readable failure explanation.
    pub message: String,
    /// Optional persisted bug-report identifier for deeper inspection.
    pub bug_report_id: Option<String>,
}

impl FileErrorDetail {
    /// Construct one failure detail from a file identity and message.
    pub(super) fn new(
        filename: impl Into<DisplayPath>,
        message: impl Into<String>,
        bug_report_id: Option<String>,
    ) -> Self {
        Self {
            filename: filename.into(),
            message: message.into(),
            bug_report_id,
        }
    }
}

/// Progress/log tracker for direct local jobs observed through repeated snapshots.
///
/// Direct mode already writes outputs locally, so the CLI only needs to:
/// - update the progress renderer from the latest job snapshot
/// - print each newly terminal file once
#[derive(Default)]
pub(super) struct DirectProgressTracker {
    seen_terminal_files: HashSet<String>,
}

impl DirectProgressTracker {
    /// Project one job snapshot into the CLI progress sink.
    pub(super) fn observe(&mut self, progress: &dyn ProgressSink, info: &JobInfo) {
        progress.update(info.completed_files.max(0) as u64, &info.file_statuses);
        if let Some(ref bp) = info.batch_progress {
            progress.update_batch_progress(bp);
        }

        for entry in &info.file_statuses {
            let filename = entry.filename.to_string();
            if self.seen_terminal_files.contains(&filename) {
                continue;
            }

            match entry.status {
                FileStatusKind::Done => {
                    progress.log_done(entry.filename.as_ref());
                    self.seen_terminal_files.insert(filename);
                }
                FileStatusKind::Error => {
                    let error_msg = entry
                        .error
                        .clone()
                        .unwrap_or_else(|| "unknown error".into());
                    progress.log_error(entry.filename.as_ref(), &error_msg);
                    self.seen_terminal_files.insert(filename);
                }
                _ => {}
            }
        }
    }
}

/// Collect CLI-visible file errors from a final job snapshot.
pub(super) fn file_error_details(info: &JobInfo) -> Vec<FileErrorDetail> {
    info.file_statuses
        .iter()
        .filter(|entry| entry.status == FileStatusKind::Error)
        .map(|entry| {
            FileErrorDetail::new(
                entry.filename.clone(),
                entry
                    .error
                    .clone()
                    .unwrap_or_else(|| "unknown error".into()),
                entry.bug_report_id.clone(),
            )
        })
        .collect()
}

/// Poll a single-server job, writing results incrementally as files complete.
#[allow(clippy::too_many_arguments)]
pub(super) async fn poll_and_write_incrementally(
    client: &BatchalignClient,
    server_url: &str,
    job_id: &JobId,
    total_files: u64,
    result_map: &HashMap<String, PathBuf>,
    out_dir: &Path,
    _command: &str,
    progress: &dyn ProgressSink,
) -> Result<(), CliError> {
    let mut written_files: HashSet<String> = HashSet::new();
    let mut written_count: u64 = 0;
    let mut error_details: Vec<FileErrorDetail> = Vec::new();
    let mut consecutive_failures: u32 = 0;
    let mut poll_interval = POLL_MIN;
    let mut last_completed: i64 = 0;
    let mut last_health_poll = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or_else(std::time::Instant::now);
    let mut last_execution_stage: Option<String> = None;

    loop {
        match client.get_job(server_url, job_id).await {
            Ok(info) => {
                consecutive_failures = 0;

                // Show execution plan stage transitions for staged remote jobs.
                if let Some(plan) = &info.execution_plan {
                    use crate::types::execution_plan::ExecutionStage;
                    let stage_label = plan.stage.to_string();
                    if last_execution_stage.as_deref() != Some(&stage_label) {
                        let host = &plan.execution_host;
                        let msg = match plan.stage {
                            ExecutionStage::Staging => format!("Staging files to {host}..."),
                            ExecutionStage::Executing => {
                                let rid = plan
                                    .remote_job_id
                                    .as_ref()
                                    .map(|id| format!(" (remote job {id})"))
                                    .unwrap_or_default();
                                format!("Executing on {host}{rid}...")
                            }
                            ExecutionStage::CopyingBack => "Copying results back...".into(),
                            ExecutionStage::Done => "Done.".into(),
                            ExecutionStage::Failed => "Staged remote execution failed.".into(),
                        };
                        eprintln!("  [{stage_label}] {msg}");
                        last_execution_stage = Some(stage_label);
                    }
                }

                for entry in &info.file_statuses {
                    let fn_ = &entry.filename;
                    if written_files.contains(&**fn_) {
                        continue;
                    }

                    if entry.status == FileStatusKind::Done {
                        match client.get_file_result(server_url, job_id, fn_).await {
                            Ok(result) => {
                                match output::write_result(&result, result_map, out_dir) {
                                    Ok(true) => {
                                        written_count += 1;
                                        progress.log_done(fn_);
                                    }
                                    Ok(false) => {
                                        let error_msg = result.error.unwrap_or_default();
                                        progress.log_error(fn_, &error_msg);
                                        error_details.push(FileErrorDetail::new(
                                            fn_.clone(),
                                            error_msg,
                                            entry.bug_report_id.clone(),
                                        ));
                                    }
                                    Err(e) => {
                                        let error_msg = format!("{e}");
                                        progress.log_error(fn_, &error_msg);
                                        error_details.push(FileErrorDetail::new(
                                            fn_.clone(),
                                            error_msg,
                                            entry.bug_report_id.clone(),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("{e}");
                                progress.log_error(fn_, &error_msg);
                                error_details.push(FileErrorDetail::new(
                                    fn_.clone(),
                                    error_msg,
                                    entry.bug_report_id.clone(),
                                ));
                            }
                        }
                        written_files.insert(fn_.to_string());
                    } else if entry.status == FileStatusKind::Error {
                        written_files.insert(fn_.to_string());
                        let error_msg = entry
                            .error
                            .clone()
                            .unwrap_or_else(|| "unknown error".into());
                        progress.log_error(fn_, &error_msg);
                        error_details.push(FileErrorDetail::new(
                            fn_.clone(),
                            error_msg,
                            entry.bug_report_id.clone(),
                        ));
                    }
                }

                let done_so_far = written_count + error_details.len() as u64;
                progress.update(done_so_far, &info.file_statuses);

                if info.status.is_terminal() {
                    if info.status == JobStatus::Cancelled
                        && let Some(receipt) = cancelled_receipt_from(&info)
                    {
                        progress.send_cancelled_receipt(receipt);
                    }
                    progress.finish();
                    return finish_terminal_job(&info, &error_details, total_files, out_dir);
                }

                let current = info.completed_files;
                if current > last_completed {
                    poll_interval = POLL_MIN;
                    last_completed = current;
                } else {
                    poll_interval = (poll_interval + POLL_STEP).min(POLL_MAX);
                }
            }
            Err(err @ CliError::JobLost { .. }) => {
                progress.finish();
                return Err(err);
            }
            Err(_) => {
                consecutive_failures += 1;
                if consecutive_failures >= MAX_POLL_FAILURES {
                    progress.finish();
                    return Err(CliError::PollExhausted {
                        attempts: MAX_POLL_FAILURES,
                    });
                }
            }
        }

        // Poll health on a slower cadence (~5s) for TUI metrics
        if last_health_poll.elapsed() >= std::time::Duration::from_secs(5) {
            if let Ok(h) = client.health_check(server_url).await {
                progress.update_health(&h);
            }
            last_health_poll = std::time::Instant::now();
        }

        tokio::time::sleep(Duration::from_secs_f64(poll_interval)).await;
    }
}

/// Resolve whether the CLI should auto-open a submitted dashboard URL.
///
/// The public CLI flag is the main user-facing control, while the
/// `BATCHALIGN_NO_BROWSER` environment variable remains a hidden backstop for
/// tests and harnesses that must suppress browser launch.
pub(super) fn dashboard_auto_open_enabled(cli_enabled: bool, no_browser_env: bool) -> bool {
    cli_enabled && !no_browser_env
}

/// Launch the submitted job's dashboard URL in the local browser when enabled.
///
/// Only opens the browser on macOS when all of:
/// - the CLI flag is enabled (default on, `--no-open-dashboard` suppresses)
/// - `BATCHALIGN_NO_BROWSER` env var is not set
/// - stderr is a TTY (interactive terminal, not piped/cron/ssh)
pub(super) fn maybe_open_dashboard(dashboard_url: &str, cli_enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        use std::io::IsTerminal;

        if !dashboard_auto_open_enabled(
            cli_enabled,
            std::env::var_os("BATCHALIGN_NO_BROWSER").is_some(),
        ) {
            return;
        }

        // Only open in interactive sessions — not cron, CI, ssh, or piped output.
        if !std::io::stderr().is_terminal() {
            return;
        }

        let _ = std::process::Command::new("open")
            .arg(dashboard_url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (dashboard_url, cli_enabled);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn terminal_job_detail(info: &JobInfo, error_details: &[FileErrorDetail]) -> String {
    if let Some(job_error) = info.error.as_ref().filter(|s| !s.trim().is_empty()) {
        return job_error.clone();
    }
    if let Some(detail) = error_details.first() {
        let first_line = detail.message.lines().next().unwrap_or("unknown error");
        let filename = detail.filename.as_ref();
        return format!("{filename}: {first_line}");
    }
    match info.status {
        JobStatus::Cancelled => "job was cancelled".into(),
        JobStatus::Interrupted => "job was interrupted".into(),
        JobStatus::Failed => "job failed without a reported error".into(),
        JobStatus::WritebackFailed => {
            "remote execution succeeded but copying results back failed".into()
        }
        JobStatus::Completed | JobStatus::Queued | JobStatus::Running => {
            "job reported success without a detailed error".into()
        }
    }
}

fn print_job_terminal_failure(
    info: &JobInfo,
    error_details: &[FileErrorDetail],
    total_files: u64,
    out_dir: &Path,
) {
    if !error_details.is_empty() {
        print_failure_summary(&info.file_statuses, error_details, total_files, out_dir);
        if let Some(job_error) = info.error.as_ref().filter(|s| !s.trim().is_empty()) {
            eprintln!("job error: {job_error}");
        }
        return;
    }

    let detail = terminal_job_detail(info, error_details);
    let bar = "\u{2501}".repeat(50);
    eprintln!("\n{bar}");
    eprintln!("  JOB {}: {}", info.status, detail);
    eprintln!("{bar}\n");
}

pub(super) fn finish_terminal_job(
    info: &JobInfo,
    error_details: &[FileErrorDetail],
    total_files: u64,
    out_dir: &Path,
) -> Result<(), CliError> {
    let clean_success = info.status == JobStatus::Completed
        && error_details.is_empty()
        && info.error.as_ref().is_none_or(|s| s.trim().is_empty());
    if clean_success {
        print_failure_summary(&info.file_statuses, error_details, total_files, out_dir);
        return Ok(());
    }

    let detail = terminal_job_detail(info, error_details);
    print_job_terminal_failure(info, error_details, total_files, out_dir);
    if info.status == JobStatus::Cancelled
        && let Some(receipt) = cancelled_receipt_from(info)
    {
        eprint_cancellation_receipt(&receipt);
    }
    Err(CliError::JobFailed {
        job_id: info.job_id.clone(),
        status: info.status.to_string(),
        detail,
    })
}

/// Print the cancellation receipt to stderr so it lives in the
/// user's terminal scrollback after the TUI exits. Matches the
/// banner the TUI showed; gives the user a second place to see
/// "yes you cancelled this" when the email comes in tomorrow.
fn eprint_cancellation_receipt(receipt: &crate::cli::tui::app::CancelledReceipt) {
    let host = receipt.host.as_deref().unwrap_or("(unknown host)");
    eprintln!(
        "  CANCELLATION RECEIPT: source={}  host={}  at={}",
        receipt.source, host, receipt.at_iso
    );
    if let Some(reason) = &receipt.reason {
        eprintln!("    reason: {reason}");
    }
    eprintln!(
        "    Run `batchalign3 jobs cancellations <job_id> --server <url>` for full audit history."
    );
}

/// Command-specific file filtering after extension-based discovery.
///
/// AVQI operates on paired `.cs/.sv` files and should only process the
/// continuous-speech side (`*.cs.<ext>`). The sustained-vowel partner is
/// resolved server-side by filename convention. Compare uses `*.gold.cha`
/// companions as references and should not submit them as primary inputs.
pub(super) fn filter_files_for_command(
    command: ReleasedCommand,
    files: Vec<PathBuf>,
    outputs: Vec<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut kept_files = Vec::new();
    let mut kept_outputs = Vec::new();
    for (f, o) in files.into_iter().zip(outputs) {
        let name = f.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        let keep = match command {
            ReleasedCommand::Avqi => lower.contains(".cs."),
            ReleasedCommand::Compare => !lower.ends_with(".gold.cha"),
            _ => true,
        };
        if keep {
            kept_files.push(f);
            kept_outputs.push(o);
        }
    }

    (kept_files, kept_outputs)
}

/// Classify files into CHAT payloads and media filenames.
pub(super) fn classify_files(
    files: &[PathBuf],
    server_names: &[String],
) -> Result<(Vec<FilePayload>, Vec<String>), CliError> {
    let mut payloads = Vec::new();
    let mut media_names = Vec::new();

    for (path, name) in files.iter().zip(server_names.iter()) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "cha" {
            let content = std::fs::read_to_string(path)?;
            payloads.push(FilePayload {
                filename: crate::api::DisplayPath::from(name.as_str()),
                content,
            });
        } else {
            media_names.push(name.clone());
        }
    }

    Ok((payloads, media_names))
}

/// Read a lexicon CSV and inject as MWT data into typed options.
pub(super) fn inject_lexicon(
    opts: &mut crate::options::CommandOptions,
    lexicon: Option<&str>,
) -> Result<(), CliError> {
    let Some(path) = lexicon else {
        return Ok(());
    };
    let path = path.trim();
    if path.is_empty() {
        return Ok(());
    }

    let content = std::fs::read_to_string(path)?;
    let mwt = &mut opts.common_mut().mwt;
    for line in content.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            let key = parts[0].trim().to_string();
            let vals: Vec<String> = parts[1..].iter().map(|s| s.trim().to_string()).collect();
            mwt.insert(key, vals);
        }
    }
    Ok(())
}

/// Tally of terminal file states for the end-of-job results line.
///
/// Pure projection of `file_statuses` — no other source. Lets the
/// display layer report what actually happened (e.g., 6 done + 1
/// errored + 93 still-queued after a cancel) instead of inferring
/// `succeeded = total - errors.len()`, which silently miscounts
/// queued-but-never-dispatched files as successes.
///
/// Pre-fix bug (2026-04-26): on a 100-file cancel with 6 done, 1
/// errored, 93 queued, the TUI footer reported "99 succeeded" because
/// the only signal was the error count. An operator report on that
/// run quoted the wrong number as evidence of success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TerminalCounts {
    pub succeeded: u64,
    pub failed: u64,
    pub not_started: u64,
    pub total: u64,
}

impl TerminalCounts {
    /// Tally `file_statuses` against the submission-time `total_files`.
    /// `not_started` covers everything that never reached a terminal
    /// state — `Queued`, `Processing`, `Interrupted`, plus any
    /// arithmetic gap if `file_statuses.len() < total_files` (e.g., the
    /// server only reported a subset before cancel).
    pub(super) fn from_statuses(file_statuses: &[FileStatusEntry], total_files: u64) -> Self {
        let mut succeeded: u64 = 0;
        let mut failed: u64 = 0;
        for entry in file_statuses {
            match entry.status {
                FileStatusKind::Done => succeeded += 1,
                FileStatusKind::Error => failed += 1,
                _ => {}
            }
        }
        let accounted = succeeded + failed;
        let not_started = total_files.saturating_sub(accounted);
        Self {
            succeeded,
            failed,
            not_started,
            total: total_files,
        }
    }

    /// Render the one-line `RESULTS:` summary. Includes the
    /// `not_started` segment only when the job didn't fully account
    /// for every submitted file, so a clean run still prints the
    /// familiar "N succeeded, 0 failed (of N files)".
    pub(super) fn results_line(&self) -> String {
        if self.not_started == 0 {
            format!(
                "{} succeeded, {} failed (of {} files)",
                self.succeeded, self.failed, self.total
            )
        } else {
            format!(
                "{} succeeded, {} failed, {} not started (of {} files)",
                self.succeeded, self.failed, self.not_started, self.total
            )
        }
    }
}

/// Build a `CancelledReceipt` from a `JobInfo` whose status went
/// `Cancelled`. Returns `None` when the audit-projection columns
/// aren't populated (e.g., a job cancelled by an older daemon
/// version that predates the provenance commit). The receipt feeds
/// the TUI end-of-run banner that surfaces who/when/why instead of
/// the generic "Done — N failed" suffix.
pub(super) fn cancelled_receipt_from(
    info: &JobInfo,
) -> Option<crate::cli::tui::app::CancelledReceipt> {
    let source = info.last_cancelled_source.clone()?;
    let at_iso = format_unix_ts_iso(info.last_cancelled_at?.0);
    Some(crate::cli::tui::app::CancelledReceipt {
        source,
        host: info.last_cancelled_host.clone().filter(|s| !s.is_empty()),
        reason: info.last_cancelled_reason.clone().filter(|s| !s.is_empty()),
        at_iso,
    })
}

fn format_unix_ts_iso(ts_seconds: f64) -> String {
    use chrono::DateTime;
    if !ts_seconds.is_finite() {
        return format!("invalid-unix-timestamp({ts_seconds})");
    }
    DateTime::from_timestamp_millis((ts_seconds * 1000.0).round() as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| format!("invalid-unix-timestamp({ts_seconds})"))
}

/// Print a structured failure summary.
pub(super) fn print_failure_summary(
    file_statuses: &[FileStatusEntry],
    errors: &[FileErrorDetail],
    total_files: u64,
    out_dir: &Path,
) {
    if errors.is_empty() {
        eprintln!(
            "\nAll done! {total_files} file(s) written to {}",
            out_dir.display()
        );
        return;
    }

    let counts = TerminalCounts::from_statuses(file_statuses, total_files);
    let bar = "\u{2501}".repeat(50);
    eprintln!("\n{bar}");
    eprintln!("  RESULTS: {}", counts.results_line());
    eprintln!("{bar}");

    for error in errors {
        let filename = error.filename.as_ref();
        let lines: Vec<&str> = error.message.lines().collect();
        let first_line = lines.first().copied().unwrap_or("unknown error");
        if let Some(bug_report_id) = error.bug_report_id.as_deref() {
            eprintln!("  \u{2717} {filename}: {first_line} (bug report: {bug_report_id})");
        } else {
            eprintln!("  \u{2717} {filename}: {first_line}");
        }
        // Show the last few lines of worker stderr when available (the
        // actual Python traceback or OOM message). Skip the first line
        // which is the header we already printed.
        if lines.len() > 1 {
            let tail_start = if lines.len() > 6 { lines.len() - 5 } else { 1 };
            for line in &lines[tail_start..] {
                eprintln!("    {line}");
            }
        }
    }

    eprintln!("{bar}");
    // Point users to where they can find more diagnostic detail.
    if let Some(home) = dirs::home_dir() {
        let daemon_log = home.join(".batchalign3").join("daemon.log");
        if daemon_log.exists() {
            eprintln!("  hint: server logs at {}", daemon_log.display());
        }
    }
    eprintln!();
}

/// Print stable debug handles for later human or agent inspection.
pub(super) fn print_job_debug_artifacts(artifacts: &JobDebugArtifacts) {
    eprintln!("debug job id: {}", artifacts.job_id);
    eprintln!("debug artifacts dir: {}", artifacts.staging_dir.display());
    if let Some(trace_file) = artifacts.trace_file.as_ref() {
        eprintln!("debug traces: {}", trace_file.display());
    }
    for bug_report in &artifacts.bug_report_files {
        eprintln!("debug bug report: {}", bug_report.display());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::api::{FileStatusEntry, JobId, LanguageCode3, LanguageSpec, ReleasedCommand};
    use crate::cli::progress::ProgressSink;
    use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};

    fn test_job_info(status: JobStatus, error: Option<&str>) -> JobInfo {
        JobInfo {
            job_id: JobId::from("job123"),
            status,
            command: ReleasedCommand::Benchmark,
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            source_dir: "/tmp/in".into(),
            total_files: 1,
            completed_files: 0,
            current_file: None,
            error: error.map(str::to_string),
            file_statuses: vec![FileStatusEntry {
                filename: "clip.cha".into(),
                status: FileStatusKind::Error,
                error: Some("worker failed".into()),
                error_category: None,
                error_codes: None,
                error_line: None,
                bug_report_id: None,
                started_at: None,
                finished_at: None,
                next_eligible_at: None,
                progress_current: None,
                progress_total: None,
                progress_stage: None,
                progress_label: None,
            }],
            submitted_at: None,
            submitted_by: None,
            submitted_by_name: None,
            completed_at: None,
            duration_s: None,
            next_eligible_at: None,
            num_workers: None,
            active_lease: None,
            batch_progress: None,
            control_plane: None,
            execution_plan: None,
            last_cancelled_at: None,
            last_cancelled_source: None,
            last_cancelled_host: None,
            last_cancelled_reason: None,
        }
    }

    #[derive(Default)]
    struct RecordingProgressSink {
        updates: Mutex<Vec<u64>>,
        done: Mutex<Vec<String>>,
        errors: Mutex<Vec<(String, String)>>,
        finished: Mutex<u32>,
    }

    impl ProgressSink for RecordingProgressSink {
        fn update(&self, done: u64, _file_statuses: &[FileStatusEntry]) {
            self.updates.lock().expect("updates lock").push(done);
        }

        fn log_done(&self, filename: &str) {
            self.done
                .lock()
                .expect("done lock")
                .push(filename.to_string());
        }

        fn log_error(&self, filename: &str, msg: &str) {
            self.errors
                .lock()
                .expect("errors lock")
                .push((filename.to_string(), msg.to_string()));
        }

        fn finish(&self) {
            *self.finished.lock().expect("finished lock") += 1;
        }
    }

    /// Build a `FileStatusEntry` with a given status — keeps the
    /// `TerminalCounts` tests focused on what matters (the status
    /// distribution) instead of repeating 12 default-None fields.
    fn status_entry(filename: &str, status: FileStatusKind) -> FileStatusEntry {
        FileStatusEntry {
            filename: filename.into(),
            status,
            error: None,
            error_category: None,
            error_codes: None,
            error_line: None,
            bug_report_id: None,
            started_at: None,
            finished_at: None,
            next_eligible_at: None,
            progress_current: None,
            progress_total: None,
            progress_stage: None,
            progress_label: None,
        }
    }

    /// Regression test for the 2026-04-26 Malayalam-cancel TUI bug:
    /// 100 files submitted, 6 finished, 1 errored, 93 still queued
    /// when the user cancelled. The pre-fix display reported "99
    /// succeeded, 1 failed" — `total - errors.len()` arithmetic — even
    /// though only 6 actually finished. After fix, the line includes
    /// the `not_started` bucket and the `succeeded` count reflects
    /// reality.
    #[test]
    fn cancelled_with_queued_files_reports_actual_done_count() {
        let mut statuses: Vec<FileStatusEntry> = Vec::with_capacity(100);
        for i in 0..6 {
            statuses.push(status_entry(&format!("done-{i}.cha"), FileStatusKind::Done));
        }
        statuses.push(status_entry("errored.cha", FileStatusKind::Error));
        for i in 0..93 {
            statuses.push(status_entry(
                &format!("queued-{i}.cha"),
                FileStatusKind::Queued,
            ));
        }

        let counts = TerminalCounts::from_statuses(&statuses, 100);
        assert_eq!(counts.succeeded, 6, "only the 6 Done files succeeded");
        assert_eq!(counts.failed, 1, "exactly one errored file");
        assert_eq!(
            counts.not_started, 93,
            "93 files were queued at cancel and never reached terminal state"
        );

        let line = counts.results_line();
        assert!(
            line.contains("6 succeeded"),
            "RESULTS line must report actual done count, not total-minus-errors; got {line:?}"
        );
        assert!(
            line.contains("93 not started"),
            "RESULTS line must surface the queued-at-cancel files; got {line:?}"
        );
        assert!(
            !line.contains("99 succeeded"),
            "RESULTS line must not perpetuate the pre-fix arithmetic; got {line:?}"
        );
    }

    /// Clean run (every submitted file accounted for as Done or Error)
    /// preserves the familiar one-line summary without a `not started`
    /// segment — that segment is for partial-completion cases only.
    #[test]
    fn clean_run_omits_not_started_segment() {
        let statuses = vec![
            status_entry("a.cha", FileStatusKind::Done),
            status_entry("b.cha", FileStatusKind::Done),
            status_entry("c.cha", FileStatusKind::Error),
        ];

        let line = TerminalCounts::from_statuses(&statuses, 3).results_line();
        assert_eq!(line, "2 succeeded, 1 failed (of 3 files)");
    }

    /// `Processing` and `Interrupted` belong in `not_started` (i.e.,
    /// "did not reach a terminal state") so a daemon crash mid-run
    /// reports the right counts.
    #[test]
    fn non_terminal_kinds_count_as_not_started() {
        let statuses = vec![
            status_entry("a.cha", FileStatusKind::Done),
            status_entry("b.cha", FileStatusKind::Processing),
            status_entry("c.cha", FileStatusKind::Interrupted),
            status_entry("d.cha", FileStatusKind::Queued),
        ];
        let counts = TerminalCounts::from_statuses(&statuses, 4);
        assert_eq!(counts.succeeded, 1);
        assert_eq!(counts.failed, 0);
        assert_eq!(counts.not_started, 3);
    }

    #[test]
    fn dashboard_auto_open_enabled_when_cli_allows_and_env_clear() {
        assert!(dashboard_auto_open_enabled(true, false));
    }

    #[test]
    fn dashboard_auto_open_disabled_when_cli_disables() {
        assert!(!dashboard_auto_open_enabled(false, false));
    }

    #[test]
    fn dashboard_auto_open_disabled_by_env_backstop() {
        assert!(!dashboard_auto_open_enabled(true, true));
    }

    #[test]
    fn classify_cha_vs_media() {
        let dir = tempfile::tempdir().unwrap();
        let cha = dir.path().join("test.cha");
        std::fs::write(&cha, "@Begin\n@End\n").unwrap();

        let files = vec![cha, PathBuf::from("audio.mp3")];
        let names = vec!["test.cha".to_string(), "audio.mp3".to_string()];

        let (payloads, media) = classify_files(&files, &names).unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].filename, "test.cha");
        assert_eq!(media, vec!["audio.mp3"]);
    }

    #[test]
    fn filter_avqi_keeps_only_cs_files() {
        let files = vec![
            PathBuf::from("sample.cs.wav"),
            PathBuf::from("sample.sv.wav"),
            PathBuf::from("other.CS.MP3"),
        ];
        let outputs = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")];

        let (f, o) = filter_files_for_command(ReleasedCommand::Avqi, files, outputs);
        assert_eq!(f.len(), 2);
        assert_eq!(o.len(), 2);
        assert!(f[0].to_string_lossy().contains(".cs."));
        assert!(f[1].to_string_lossy().to_ascii_lowercase().contains(".cs."));
    }

    #[test]
    fn filter_compare_skips_gold_chat_companions() {
        let files = vec![
            PathBuf::from("sample.cha"),
            PathBuf::from("sample.gold.cha"),
            PathBuf::from("other.GOLD.CHA"),
            PathBuf::from("other.cha"),
        ];
        let outputs = vec![
            PathBuf::from("a"),
            PathBuf::from("b"),
            PathBuf::from("c"),
            PathBuf::from("d"),
        ];

        let (f, o) = filter_files_for_command(ReleasedCommand::Compare, files, outputs);
        assert_eq!(f.len(), 2);
        assert_eq!(o.len(), 2);
        assert!(f.iter().all(|path| {
            !path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".gold.cha")
        }));
    }

    #[test]
    fn inject_lexicon_csv() {
        use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};

        let dir = tempfile::tempdir().unwrap();
        let lex = dir.path().join("lex.csv");
        std::fs::write(&lex, "gonna,going,to\nwanna,want,to\n").unwrap();

        let mut opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        });
        inject_lexicon(&mut opts, Some(lex.to_str().unwrap())).unwrap();

        let mwt = &opts.common().mwt;
        assert_eq!(mwt.len(), 2);
        assert!(mwt.contains_key("gonna"));
        assert_eq!(mwt["gonna"], vec!["going", "to"]);
    }

    #[test]
    fn capability_check_allows_test_echo() {
        use crate::ReleasedCommand;

        let caps = vec!["test-echo".to_string()];
        assert!(super::super::server_supports_command(
            &caps,
            ReleasedCommand::Morphotag
        ));
    }

    #[test]
    fn capability_check_rejects_missing_command() {
        use crate::ReleasedCommand;

        let caps = vec!["align".to_string(), "transcribe".to_string()];
        assert!(!super::super::server_supports_command(
            &caps,
            ReleasedCommand::Morphotag
        ));
    }

    #[test]
    fn inject_lexicon_missing_file() {
        use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};

        let mut opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        });
        let result = inject_lexicon(&mut opts, Some("/nonexistent/lexicon.csv"));
        assert!(result.is_err());
    }

    #[test]
    fn inject_lexicon_empty_path() {
        let mut opts = CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        });
        inject_lexicon(&mut opts, Some("  ")).unwrap();
        // Empty/whitespace path is a no-op
        assert!(opts.common().mwt.is_empty());
    }

    #[test]
    fn finish_terminal_job_accepts_clean_completed_job() {
        let mut info = test_job_info(JobStatus::Completed, None);
        info.file_statuses[0].status = FileStatusKind::Done;
        info.file_statuses[0].error = None;
        info.completed_files = 1;
        let out_dir = tempfile::tempdir().unwrap();

        let result = finish_terminal_job(&info, &[], 1, out_dir.path());

        assert!(result.is_ok());
    }

    #[test]
    fn finish_terminal_job_rejects_failed_job_status() {
        let info = test_job_info(JobStatus::Failed, Some("worker pool exploded"));
        let out_dir = tempfile::tempdir().unwrap();

        let result = finish_terminal_job(&info, &[], 1, out_dir.path());

        match result {
            Err(CliError::JobFailed {
                job_id,
                status,
                detail,
            }) => {
                assert_eq!(job_id, "job123");
                assert_eq!(status, "failed");
                assert_eq!(detail, "worker pool exploded");
            }
            other => panic!("expected JobFailed, got {other:?}"),
        }
    }

    #[test]
    fn finish_terminal_job_rejects_completed_job_with_file_errors() {
        let info = test_job_info(JobStatus::Completed, None);
        let out_dir = tempfile::tempdir().unwrap();
        let errors = vec![FileErrorDetail::new(
            "clip.cha",
            "decoder failed\ntrace",
            None,
        )];

        let result = finish_terminal_job(&info, &errors, 1, out_dir.path());

        match result {
            Err(CliError::JobFailed {
                job_id,
                status,
                detail,
            }) => {
                assert_eq!(job_id, "job123");
                assert_eq!(status, "completed");
                assert_eq!(detail, "clip.cha: decoder failed");
            }
            other => panic!("expected JobFailed, got {other:?}"),
        }
    }

    #[test]
    fn direct_progress_tracker_logs_new_terminal_files_once() {
        let sink = RecordingProgressSink::default();
        let mut tracker = DirectProgressTracker::default();
        let mut info = test_job_info(JobStatus::Running, None);
        info.file_statuses = vec![FileStatusEntry {
            filename: "a.cha".into(),
            status: FileStatusKind::Processing,
            error: None,
            error_category: None,
            error_codes: None,
            error_line: None,
            bug_report_id: None,
            started_at: None,
            finished_at: None,
            next_eligible_at: None,
            progress_current: Some(1),
            progress_total: Some(3),
            progress_stage: None,
            progress_label: Some("processing".into()),
        }];
        info.completed_files = 0;

        tracker.observe(&sink, &info);

        info.file_statuses = vec![
            FileStatusEntry {
                filename: "a.cha".into(),
                status: FileStatusKind::Done,
                error: None,
                error_category: None,
                error_codes: None,
                error_line: None,
                bug_report_id: None,
                started_at: None,
                finished_at: None,
                next_eligible_at: None,
                progress_current: None,
                progress_total: None,
                progress_stage: None,
                progress_label: None,
            },
            FileStatusEntry {
                filename: "b.cha".into(),
                status: FileStatusKind::Error,
                error: Some("decoder failed".into()),
                error_category: None,
                error_codes: None,
                error_line: None,
                bug_report_id: None,
                started_at: None,
                finished_at: None,
                next_eligible_at: None,
                progress_current: None,
                progress_total: None,
                progress_stage: None,
                progress_label: None,
            },
        ];
        info.completed_files = 2;

        tracker.observe(&sink, &info);
        tracker.observe(&sink, &info);

        assert_eq!(*sink.updates.lock().expect("updates lock"), vec![0, 2, 2]);
        assert_eq!(*sink.done.lock().expect("done lock"), vec!["a.cha"]);
        assert_eq!(
            *sink.errors.lock().expect("errors lock"),
            vec![("b.cha".into(), "decoder failed".into())]
        );
    }
}
