//! `batchalign3 jobs` -- inspect remote jobs or local debug artifacts by job ID.
//!
//! This module implements the `jobs` subcommand, which lets users inspect
//! processing jobs without opening the dashboard. It operates in three modes:
//!
//! - **Single-server list** -- When `--server` is given, lists all jobs on
//!   that server with their status, command, and file progress counts.
//!
//! - **Single-job detail** -- When a job ID is provided as a positional argument,
//!   fetches and displays the full job record including per-file statuses and
//!   error messages.
//!
//! - **Local debug inspection** -- When `--server` is omitted but a job ID is
//!   provided, inspects the local runtime state under `~/.batchalign3/jobs/`
//!   (or `BATCHALIGN_STATE_DIR`) and reports stable artifact handles for later
//!   human or agent inspection.

use std::fs;
use std::path::{Path, PathBuf};

use crate::api::JobId;
use crate::config::RuntimeLayout;
use crate::debug_artifacts::JobDebugArtifacts;
use serde::Serialize;

use crate::api::CancellationRecord;
use crate::cli::args::{JobsAction, JobsArgs, JobsCancellationsArgs};
use crate::cli::client::BatchalignClient;
use crate::cli::error::CliError;
use std::fmt::Write as _;

const DEBUG_ARTIFACTS_FILENAME: &str = "debug-artifacts.json";
const DEBUG_TRACES_FILENAME: &str = "debug-traces.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LocalJobInspection {
    job_id: String,
    staging_dir: PathBuf,
    debug_summary_file: Option<PathBuf>,
    trace_file: Option<PathBuf>,
    bug_report_ids: Vec<String>,
    bug_report_files: Vec<PathBuf>,
    persisted_summary: bool,
}

/// Execute the `jobs` command. Dispatches to a sub-action when one
/// is given (`jobs cancellations <id>`); otherwise falls through to
/// the legacy positional `jobs <id>` / `jobs --server X` shape.
pub async fn run(args: &JobsArgs) -> Result<(), CliError> {
    if let Some(action) = &args.action {
        return match action {
            JobsAction::Cancellations(a) => run_cancellations(a).await,
        };
    }

    if let Some(ref server) = args.server {
        let client = BatchalignClient::new()?;
        let server = server.trim_end_matches('/');

        if let Some(ref id) = args.job_id {
            show_job(&client, server, &JobId::from(id.as_str()), args.json).await
        } else {
            list_jobs(&client, server, args.json).await
        }
    } else if let Some(ref job_id) = args.job_id {
        let layout = RuntimeLayout::from_env();
        let inspection = inspect_local_job(&layout, job_id)?;
        print_local_job(&inspection, args.json)
    } else {
        Err(CliError::InvalidArgument(
            "JOB_ID required when --server is omitted".into(),
        ))
    }
}

/// Print every cancel attempt against `args.job_id` from the
/// server's `cancellations` audit table. After-the-fact verification
/// — when a user reports "I didn't cancel that job," this is the
/// answer.
async fn run_cancellations(args: &JobsCancellationsArgs) -> Result<(), CliError> {
    let server = args
        .server
        .as_deref()
        .ok_or_else(|| {
            CliError::InvalidArgument("--server (or BATCHALIGN_SERVER) required".into())
        })?
        .trim_end_matches('/');

    let client = BatchalignClient::new()?;
    let job_id = JobId::from(args.job_id.as_str());
    let records = client.list_cancellations(server, &job_id).await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&records)?);
    } else {
        print!("{}", format_cancellations(&job_id, &records));
    }
    Ok(())
}

/// Render the audit history as human-readable text. Pure function so
/// tests can assert against the rendered output.
pub(crate) fn format_cancellations(job_id: &JobId, records: &[CancellationRecord]) -> String {
    let mut out = String::new();
    if records.is_empty() {
        let _ = writeln!(out, "No cancel attempts recorded for job {job_id}.");
        return out;
    }
    let _ = writeln!(
        out,
        "Cancellations for job {job_id} ({} total):",
        records.len()
    );
    for (idx, rec) in records.iter().enumerate() {
        let n = idx + 1;
        let ts = format_unix_ts(rec.requested_at.0);
        let host = rec.host.as_ref().map(|h| h.as_ref()).unwrap_or("(unknown)");
        let pid = rec
            .pid
            .map(|p| p.0.to_string())
            .unwrap_or_else(|| "(unknown)".to_string());
        let accepted = if rec.accepted {
            "accepted"
        } else {
            "no-op (job already terminal)"
        };
        let _ = writeln!(
            out,
            "  {n}. {ts}  source={}  host={host}  pid={pid}  {accepted}",
            rec.source
        );
        if let Some(reason) = &rec.reason
            && !reason.as_ref().is_empty()
        {
            let _ = writeln!(out, "     reason: {}", reason.as_ref());
        }
        if let Some(filename) = &rec.in_flight_filename
            && !filename.as_ref().is_empty()
        {
            let _ = writeln!(out, "     in-flight at cancel: {}", filename.as_ref());
        }
    }
    out
}

fn format_unix_ts(ts_seconds: f64) -> String {
    use chrono::{DateTime, Local};
    if !ts_seconds.is_finite() {
        return format!("invalid-unix-timestamp({ts_seconds})");
    }
    DateTime::from_timestamp_millis((ts_seconds * 1000.0).round() as i64)
        .map(|dt| {
            DateTime::<Local>::from(dt)
                .format("%Y-%m-%d %H:%M:%S %Z")
                .to_string()
        })
        .unwrap_or_else(|| format!("invalid-unix-timestamp({ts_seconds})"))
}

async fn list_jobs(client: &BatchalignClient, server: &str, json: bool) -> Result<(), CliError> {
    let jobs = client.list_jobs(server).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    if jobs.is_empty() {
        eprintln!("No jobs found.");
        return Ok(());
    }

    eprintln!("\nJobs on {server}\n");
    for j in &jobs {
        let status = j.status.to_string();
        eprintln!(
            "  {}  {:<10}  {:<12}  {}/{} files",
            j.job_id, status, j.command, j.completed_files, j.total_files
        );
    }
    eprintln!();

    Ok(())
}

async fn show_job(
    client: &BatchalignClient,
    server: &str,
    job_id: &JobId,
    json: bool,
) -> Result<(), CliError> {
    let info = client.get_job(server, job_id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    eprintln!();
    eprintln!("Job {}", info.job_id);
    eprintln!("{}", "-".repeat(40));
    eprintln!("Status:   {}", info.status);
    eprintln!("Command:  {}", info.command);
    eprintln!("Files:    {}/{}", info.completed_files, info.total_files);
    if let Some(ref control_plane) = info.control_plane {
        eprintln!("Backend:  {}", control_plane.backend);
        if let Some(ref temporal) = control_plane.temporal {
            eprintln!("Workflow: {}", temporal.workflow_id);
            if let Some(ref run_id) = temporal.run_id {
                eprintln!("Run ID:   {run_id}");
            }
            if let Some(ref status) = temporal.status {
                eprintln!("WF State: {status}");
            }
            if let Some(ref task_queue) = temporal.task_queue {
                eprintln!("Queue:    {task_queue}");
            }
            if let Some(history_length) = temporal.history_length {
                eprintln!("History:  {history_length} event(s)");
            }
            if let Some(ref describe_error) = temporal.describe_error {
                eprintln!("WF Error: {describe_error}");
            }
        }
    }
    if let Some(ref current) = info.current_file {
        eprintln!("Current:  {current}");
    }
    if let Some(ref error) = info.error {
        eprintln!("Error:    {error}");
    }

    if !info.file_statuses.is_empty() {
        eprintln!();
        for entry in &info.file_statuses {
            let status = &entry.status;
            let error = entry
                .error
                .as_deref()
                .map(|e| format!(" — {e}"))
                .unwrap_or_default();
            eprintln!("  {:<30} {status}{error}", entry.filename);
        }
    }

    eprintln!();

    Ok(())
}

fn inspect_local_job(layout: &RuntimeLayout, job_id: &str) -> Result<LocalJobInspection, CliError> {
    let staging_dir = layout.jobs_dir().join(job_id);
    if !staging_dir.is_dir() {
        return Err(CliError::InvalidArgument(format!(
            "local job not found: {job_id} (expected {})",
            staging_dir.display()
        )));
    }

    let debug_summary_file = staging_dir.join(DEBUG_ARTIFACTS_FILENAME);
    if debug_summary_file.is_file() {
        let artifacts = load_debug_artifacts(&debug_summary_file)?;
        return Ok(LocalJobInspection {
            job_id: artifacts.job_id.to_string(),
            staging_dir: artifacts.staging_dir,
            debug_summary_file: Some(debug_summary_file),
            trace_file: artifacts.trace_file,
            bug_report_ids: artifacts.bug_report_ids,
            bug_report_files: artifacts.bug_report_files,
            persisted_summary: true,
        });
    }

    let trace_file = candidate_file(&staging_dir, DEBUG_TRACES_FILENAME);
    Ok(LocalJobInspection {
        job_id: job_id.to_string(),
        staging_dir,
        debug_summary_file: None,
        trace_file,
        bug_report_ids: Vec::new(),
        bug_report_files: Vec::new(),
        persisted_summary: false,
    })
}

fn load_debug_artifacts(path: &Path) -> Result<JobDebugArtifacts, CliError> {
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

fn candidate_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let path = dir.join(name);
    path.is_file().then_some(path)
}

fn print_local_job(inspection: &LocalJobInspection, json: bool) -> Result<(), CliError> {
    if json {
        println!("{}", serde_json::to_string_pretty(inspection)?);
        return Ok(());
    }

    eprintln!();
    eprintln!("Local job {}", inspection.job_id);
    eprintln!("{}", "-".repeat(40));
    eprintln!("Artifacts: {}", inspection.staging_dir.display());
    eprintln!(
        "Summary:   {}",
        if inspection.persisted_summary {
            "persisted debug-artifacts.json"
        } else {
            "staging-dir fallback"
        }
    );
    if let Some(ref summary) = inspection.debug_summary_file {
        eprintln!("Debug:     {}", summary.display());
    }
    if let Some(ref trace_file) = inspection.trace_file {
        eprintln!("Traces:    {}", trace_file.display());
    }
    for bug_report_id in &inspection.bug_report_ids {
        eprintln!("Bug ID:    {bug_report_id}");
    }
    for bug_report_file in &inspection.bug_report_files {
        eprintln!("Bug file:  {}", bug_report_file.display());
    }
    eprintln!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::api::{
        CallerHost, CallerPid, CancelReason, CancelSource, DisplayPath, JobId, UnixTimestamp,
    };

    fn cancel_record(
        id: i64,
        ts: f64,
        source: CancelSource,
        host: Option<&str>,
        pid: Option<u32>,
        reason: Option<&str>,
        in_flight: Option<&str>,
        accepted: bool,
    ) -> CancellationRecord {
        CancellationRecord {
            id,
            job_id: JobId::from("test-job"),
            requested_at: UnixTimestamp(ts),
            source,
            host: host.map(|s| CallerHost::from(s.to_string())),
            pid: pid.map(CallerPid),
            reason: reason.map(|s| CancelReason::from(s.to_string())),
            correlation_id: None,
            in_flight_filename: in_flight.map(|s| DisplayPath::from(s)),
            accepted,
        }
    }

    /// Empty audit history: explicit "none recorded" message instead of
    /// silent blank output, so a user can tell "no cancels happened"
    /// from "the command failed."
    #[test]
    fn format_cancellations_empty_history_says_so() {
        let out = format_cancellations(&JobId::from("nojob"), &[]);
        assert!(out.contains("No cancel attempts recorded"));
        assert!(out.contains("nojob"));
    }

    /// 2026-04-26 scenario: TUI cancel with full provenance.
    /// The output must surface enough that a user can verify "yes
    /// I did press c-y from my laptop at that time" without
    /// querying the DB.
    #[test]
    fn format_cancellations_tui_record_renders_full_metadata() {
        let rec = cancel_record(
            1,
            1714124053.0,
            CancelSource::Tui,
            Some("test-laptop"),
            Some(12345),
            Some("user-pressed-cancel"),
            Some("L1-class-L1/2/P18/L1cq.mp3"),
            true,
        );
        let out = format_cancellations(&JobId::from("e4115057-5ea"), &[rec]);
        assert!(out.contains("e4115057-5ea"));
        assert!(out.contains("source=tui"));
        assert!(out.contains("host=test-laptop"));
        assert!(out.contains("pid=12345"));
        assert!(out.contains("accepted"));
        assert!(out.contains("user-pressed-cancel"));
        assert!(out.contains("L1-class-L1/2/P18/L1cq.mp3"));
    }

    /// Repeat-cancel pattern (2026-04-25 Malayalam: two cancels an
    /// hour apart). Both rows render in temporal order; the second
    /// shows `no-op (job already terminal)` since the first
    /// terminalized the job.
    #[test]
    fn format_cancellations_double_cancel_shows_both_with_status() {
        let rows = vec![
            cancel_record(
                1,
                1714124053.0,
                CancelSource::Tui,
                Some("test-laptop"),
                Some(1111),
                Some("user-pressed-cancel"),
                None,
                true,
            ),
            cancel_record(
                2,
                1714124053.0 + 3600.0,
                CancelSource::Tui,
                Some("test-laptop"),
                Some(2222),
                Some("nothing-happened-pressing-again"),
                None,
                false,
            ),
        ];
        let out = format_cancellations(&JobId::from("doubled"), &rows);
        assert!(out.contains("(2 total)"));
        assert!(out.contains("pid=1111"));
        assert!(out.contains("pid=2222"));
        assert!(out.contains("no-op (job already terminal)"));
        let acc_idx = out.find("accepted").expect("accepted marker");
        let noop_idx = out.find("no-op").expect("no-op marker");
        assert!(acc_idx < noop_idx, "first row should render before second");
    }

    /// Missing optional fields render as `(unknown)` so a reader can
    /// tell "field absent" from "field empty."
    #[test]
    fn format_cancellations_missing_fields_render_unknown() {
        let rec = cancel_record(
            1,
            1714124053.0,
            CancelSource::Api,
            None,
            None,
            None,
            None,
            true,
        );
        let out = format_cancellations(&JobId::from("anon"), &[rec]);
        assert!(out.contains("source=api"));
        assert!(out.contains("host=(unknown)"));
        assert!(out.contains("pid=(unknown)"));
        assert!(!out.contains("reason:"));
        assert!(!out.contains("in-flight"));
    }

    #[test]
    fn inspect_local_job_prefers_persisted_summary() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        let job_id = "job-local-summary";
        let staging_dir = layout.jobs_dir().join(job_id);
        let bug_reports_dir = layout.bug_reports_dir();
        fs::create_dir_all(&staging_dir).expect("create staging dir");
        fs::create_dir_all(&bug_reports_dir).expect("create bug reports dir");

        let trace_file = staging_dir.join(DEBUG_TRACES_FILENAME);
        let bug_report_file = bug_reports_dir.join("bug-123.json");
        let artifacts = JobDebugArtifacts {
            job_id: JobId::from(job_id),
            staging_dir: staging_dir.clone(),
            trace_file: Some(trace_file.clone()),
            bug_report_ids: vec!["bug-123".into()],
            bug_report_files: vec![bug_report_file.clone()],
        };
        fs::write(
            staging_dir.join(DEBUG_ARTIFACTS_FILENAME),
            serde_json::to_vec_pretty(&artifacts).expect("serialize artifacts"),
        )
        .expect("write debug summary");

        let inspection = inspect_local_job(&layout, job_id).expect("inspect local job");
        assert!(inspection.persisted_summary);
        assert_eq!(inspection.job_id, job_id);
        assert_eq!(inspection.staging_dir, staging_dir);
        assert_eq!(
            inspection.debug_summary_file,
            Some(
                layout
                    .jobs_dir()
                    .join(job_id)
                    .join(DEBUG_ARTIFACTS_FILENAME)
            )
        );
        assert_eq!(inspection.trace_file, Some(trace_file));
        assert_eq!(inspection.bug_report_ids, vec!["bug-123"]);
        assert_eq!(inspection.bug_report_files, vec![bug_report_file]);
    }

    #[test]
    fn inspect_local_job_falls_back_to_staging_dir_when_summary_is_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        let job_id = "job-local-fallback";
        let staging_dir = layout.jobs_dir().join(job_id);
        fs::create_dir_all(&staging_dir).expect("create staging dir");
        let trace_file = staging_dir.join(DEBUG_TRACES_FILENAME);
        fs::write(&trace_file, "{}").expect("write trace file");

        let inspection = inspect_local_job(&layout, job_id).expect("inspect local job");
        assert!(!inspection.persisted_summary);
        assert_eq!(inspection.job_id, job_id);
        assert_eq!(inspection.staging_dir, staging_dir);
        assert_eq!(inspection.debug_summary_file, None);
        assert_eq!(inspection.trace_file, Some(trace_file));
        assert!(inspection.bug_report_ids.is_empty());
        assert!(inspection.bug_report_files.is_empty());
    }

    #[test]
    fn local_job_json_includes_summary_mode_and_bug_reports() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        let job_id = "job-local-json";
        let staging_dir = layout.jobs_dir().join(job_id);
        let bug_reports_dir = layout.bug_reports_dir();
        fs::create_dir_all(&staging_dir).expect("create staging dir");
        fs::create_dir_all(&bug_reports_dir).expect("create bug reports dir");

        let inspection = LocalJobInspection {
            job_id: job_id.into(),
            staging_dir: staging_dir.clone(),
            debug_summary_file: Some(staging_dir.join(DEBUG_ARTIFACTS_FILENAME)),
            trace_file: Some(staging_dir.join(DEBUG_TRACES_FILENAME)),
            bug_report_ids: vec!["bug-123".into()],
            bug_report_files: vec![bug_reports_dir.join("bug-123.json")],
            persisted_summary: true,
        };

        let value = serde_json::to_value(&inspection).expect("serialize inspection");
        assert_eq!(value["job_id"], job_id);
        assert_eq!(value["persisted_summary"], true);
        assert_eq!(value["bug_report_ids"][0], "bug-123");
    }
}
