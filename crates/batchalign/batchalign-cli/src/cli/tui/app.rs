//! TUI application state machine for tracking job progress.
//!
//! This module models the full state needed to render the interactive terminal
//! UI during a processing job. The state is driven by periodic HTTP poll
//! responses from the server: each poll delivers a snapshot of file statuses,
//! and [`AppState::update_from_poll`] rebuilds the directory-grouped view
//! while preserving user navigation (focus, scroll offset).
//!
//! The state machine transitions are:
//!
//! 1. **Created** -- `AppState::new()`, progress starts at zero.
//! 2. **Polling** -- `update_from_poll()` called on each tick, groups rebuilt.
//! 3. **Finished** -- `finished` set to true, TUI shows final summary.
//!
//! User input (arrow keys, tab, `e`, `c`) mutates navigation fields only;
//! it never affects the job itself (cancellation is a separate server call).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::api::{FileProgressStage, FileStatusEntry, FileStatusKind, MemoryMb};

/// Persistent record of a cancel attempt, surfaced in the TUI as
/// the end-of-run banner when a job ends with status `Cancelled`.
/// Replaces the misleading "Done — N failed" suffix that the user
/// saw after the 2026-04-26 incident — the operator read it as
/// natural completion instead of a cancel they had initiated.
///
/// Source of truth is the `cancellations` audit table, projected
/// into `JobInfo.last_cancelled_*` columns so the TUI can render
/// without a second round trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelledReceipt {
    /// Wire-format source string (`tui` / `api` / `dashboard` /
    /// `staging` / `signal`).
    pub source: String,
    /// Caller-reported host or peer-IP.
    pub host: Option<String>,
    /// Caller-reported reason text.
    pub reason: Option<String>,
    /// ISO 8601 timestamp the cancel arrived at the server.
    pub at_iso: String,
}

/// Reducer message sent from polling code into the TUI state owner.
pub enum TuiUpdate {
    /// Replace the current grouped file snapshot with the latest poll result.
    PollSnapshot {
        /// Completed file count.
        done: u64,
        /// Current server-reported file statuses.
        file_statuses: Vec<FileStatusEntry>,
    },
    /// Job ended with status `Cancelled`; surface the receipt so the
    /// end-of-run banner replaces the generic "Done" suffix.
    CancelledReceipt(CancelledReceipt),
    /// Append one error to the persistent error summary panel.
    ///
    /// Used as a fallback by the `ProgressSink` `log_error` path. The TUI
    /// also extracts error codes directly from `PollSnapshot` entries, so
    /// this variant carries no code. Prefer the poll-based path for codes.
    FileError {
        /// File that produced the error.
        filename: String,
        /// Human-readable error message.
        message: String,
    },
    /// Update server health snapshot (polled less frequently than job status).
    HealthSnapshot(ServerHealth),
    /// Mark the TUI as finished so the loop can exit after showing final state.
    Finished,
}

/// Snapshot of server health data for TUI rendering.
///
/// Populated from `HealthResponse` on a ~5-second cadence. Fields are
/// chosen to match the web dashboard's `MemoryPanel` and `WorkerProfilePanel`.
#[derive(Debug, Clone)]
pub struct ServerHealth {
    /// Number of live Python worker processes.
    pub live_workers: i64,
    /// Active worker keys (`profile:lang` or `infer:task:lang`).
    pub live_worker_keys: Vec<String>,
    /// Total physical memory in MB.
    pub system_memory_total_mb: MemoryMb,
    /// Available memory in MB (free + reclaimable).
    pub system_memory_available_mb: MemoryMb,
    /// Used memory in MB (`total - available`).
    pub system_memory_used_mb: MemoryMb,
    /// Memory gate threshold in MB from server config (0 = disabled).
    pub memory_gate_threshold_mb: MemoryMb,
    /// Background warmup lifecycle label.
    pub warmup_status: String,
    /// Host-wide memory pressure classification (healthy/guarded/constrained/critical).
    pub host_memory_pressure: String,
    /// Number of worker crashes since server start.
    pub worker_crashes: i64,
    /// Number of work-unit attempts started since server start.
    pub attempts_started: i64,
}

/// Overall TUI state, updated from poll results.
///
/// A single instance is created per job and mutated in place on every poll
/// tick. The render loop reads this struct to draw the terminal UI.
pub struct AppState {
    /// Command/progress summary for the current job.
    pub progress: JobProgressState,
    /// Directory-grouped file view plus navigation state.
    pub directories: DirectoryViewState,
    /// Error summary panel state.
    pub errors: ErrorPanelState,
    /// Confirmation and other interaction-only state.
    pub interaction: InteractionState,
    /// Latest server health snapshot (polled ~every 5s). `None` before
    /// the first health response arrives.
    pub health: Option<ServerHealth>,
    /// Whether the worker/memory metrics rows are visible.
    pub show_metrics: bool,
}

/// Job-level progress summary shown in the header.
pub struct JobProgressState {
    /// The batchalign command being run (e.g. `"morphotag"`, `"align"`).
    pub command: String,
    /// Total number of files in this job.
    pub total_files: u64,
    /// Number of files that have finished processing (success or error).
    pub completed: u64,
    /// Wall-clock instant when the job was submitted.
    pub start_time: Instant,
    /// True once the job has reached a terminal state locally.
    pub finished: bool,
    /// When the job ended with status `Cancelled`, populated with the
    /// server's audit-row metadata so the end-of-run banner can show
    /// "you pressed c-y from <host> at <time>" instead of the
    /// misleading "Done" suffix.
    pub cancelled_receipt: Option<CancelledReceipt>,
}

/// Directory-grouped file list plus current navigation state.
pub struct DirectoryViewState {
    /// Files grouped by their parent directory path, sorted lexicographically.
    pub groups: Vec<DirGroup>,
    /// Index into `groups` indicating which directory group has keyboard focus.
    pub focused_group: usize,
    /// Scroll offset within the focused group's file list.
    pub scroll_offset: usize,
    /// Monotonically increasing counter driving the spinner animation.
    pub spinner_tick: usize,
}

/// State for the collapsible error summary panel.
pub struct ErrorPanelState {
    /// Accumulated error entries shown in the summary panel.
    pub entries: Vec<ErrorEntry>,
    /// Whether the error summary panel is expanded.
    pub expanded: bool,
    /// Filenames for which error entries have already been created,
    /// preventing duplicate entries across poll ticks.
    seen_files: HashSet<String>,
}

/// Local interaction-only flags that do not affect the job itself.
pub struct InteractionState {
    /// True while the UI is showing a "press c again to confirm cancel" prompt.
    pub cancel_confirm: bool,
}

/// A group of files sharing a common parent directory.
///
/// Files are grouped by splitting each path at its last `/` separator.
/// Files without a directory component are grouped under `"."`.
/// Groups are sorted lexicographically by `dir` for stable display order.
pub struct DirGroup {
    /// The directory prefix shared by all files in this group
    /// (e.g. `"eng/Eng-NA"` or `"."` for root-level files).
    pub dir: String,

    /// All files in this directory, sorted alphabetically by filename.
    pub files: Vec<FileState>,

    /// Number of files with status `Done`. Invariant: `done_count <= files.len()`.
    pub done_count: usize,

    /// Number of files with status `Processing`. These are the files
    /// currently being worked on by a server worker.
    pub active_count: usize,

    /// Number of files with status `Error`. These files failed and will
    /// not be retried.
    pub error_count: usize,

    /// Number of files with status `Queued` or `Interrupted`.
    pub queued_count: usize,
}

/// Per-file processing status, extracted from the server's poll response.
///
/// Each file appears exactly once across all `DirGroup`s. Fields are
/// populated from `FileStatusEntry` on every poll, so values may change
/// between ticks (e.g. `status` transitions from `Queued` to `Processing`
/// to `Done`).
pub struct FileState {
    /// Filename without the directory prefix (e.g. `"test.cha"`).
    pub name: String,

    /// Full path as reported by the server (e.g. `"eng/Eng-NA/test.cha"`).
    pub full_path: String,

    /// Current processing status of this file.
    pub status: FileStatusKind,

    /// Wall-clock processing time in seconds, computed from the server's
    /// `started_at` and `finished_at` timestamps. `None` while the file
    /// is still queued or processing.
    pub duration_s: Option<f64>,

    /// Unix timestamp when processing started. Used to compute elapsed
    /// time for files that are still being processed.
    pub started_at: Option<f64>,

    /// Current step in a multi-step file operation (e.g. Rev.AI polling).
    /// `None` if the server does not report sub-file progress.
    pub progress_current: Option<i64>,

    /// Total number of steps for this file. `None` if unknown.
    pub progress_total: Option<i64>,

    /// Human-readable label for the current progress step
    /// (e.g. `"uploading"`, `"aligning"`). `None` if not reported.
    pub progress_label: Option<String>,

    /// Typed pipeline stage for the current step. Used to render the
    /// 5-segment phase indicator. `None` for queued/done/error files.
    pub progress_stage: Option<FileProgressStage>,

    /// Error message if the file failed. `None` for successful or
    /// in-progress files.
    pub error_msg: Option<String>,

    /// Structured error codes attached to the failure (e.g. `["E362"]`).
    /// Empty vec for files that have not failed.
    pub error_codes: Vec<String>,
}

/// An error entry displayed in the collapsible error summary panel.
///
/// Error entries are appended as they are discovered during polling and
/// are never removed. They provide a persistent record of every failure
/// in the current job, even after the file list has scrolled past.
pub struct ErrorEntry {
    /// The filename that produced this error (display name, not full path).
    pub filename: String,

    /// Structured error code if available (e.g. `"E362"`). `None` for
    /// errors that do not carry a CHAT-spec error code.
    pub code: Option<String>,

    /// Human-readable error description from the server or worker.
    pub message: String,
}

impl AppState {
    /// Create initial state for a new job.
    pub fn new(total_files: u64, command: &str) -> Self {
        Self {
            progress: JobProgressState {
                command: command.to_string(),
                total_files,
                completed: 0,
                start_time: Instant::now(),
                finished: false,
                cancelled_receipt: None,
            },
            directories: DirectoryViewState {
                groups: Vec::new(),
                focused_group: 0,
                scroll_offset: 0,
                spinner_tick: 0,
            },
            errors: ErrorPanelState {
                entries: Vec::new(),
                expanded: false,
                seen_files: HashSet::new(),
            },
            interaction: InteractionState {
                cancel_confirm: false,
            },
            health: None,
            show_metrics: true,
        }
    }

    /// Update state from poll results.
    pub fn update_from_poll(&mut self, done: u64, file_statuses: &[FileStatusEntry]) {
        self.progress.completed = done;

        // Group files by parent directory
        let mut dir_map: HashMap<String, Vec<FileState>> = HashMap::new();

        for entry in file_statuses {
            let (dir, name) = split_dir_file(&entry.filename);
            let status = entry.status;

            let duration_s = match (entry.started_at, entry.finished_at) {
                (Some(start), Some(end)) => Some(end.0 - start.0),
                _ => None,
            };

            let file_state = FileState {
                name: name.to_string(),
                full_path: entry.filename.to_string(),
                status,
                duration_s,
                started_at: entry.started_at.map(|t| t.0),
                progress_current: entry.progress_current,
                progress_total: entry.progress_total,
                progress_label: entry.progress_label.clone(),
                progress_stage: entry.progress_stage,
                error_msg: entry.error.clone(),
                error_codes: entry.error_codes.clone().unwrap_or_default(),
            };

            dir_map.entry(dir.to_string()).or_default().push(file_state);
        }

        // Build sorted groups, preserving UI state
        let mut groups: Vec<DirGroup> = dir_map
            .into_iter()
            .map(|(dir, mut files)| {
                files.sort_by(|a, b| a.name.cmp(&b.name));
                let done_count = files
                    .iter()
                    .filter(|f| f.status == FileStatusKind::Done)
                    .count();
                let active_count = files
                    .iter()
                    .filter(|f| f.status == FileStatusKind::Processing)
                    .count();
                let error_count = files
                    .iter()
                    .filter(|f| f.status == FileStatusKind::Error)
                    .count();
                let queued_count = files
                    .iter()
                    .filter(|f| {
                        f.status == FileStatusKind::Queued
                            || f.status == FileStatusKind::Interrupted
                    })
                    .count();
                DirGroup {
                    dir,
                    files,
                    done_count,
                    active_count,
                    error_count,
                    queued_count,
                }
            })
            .collect();
        groups.sort_by(|a, b| a.dir.cmp(&b.dir));

        self.directories.groups = groups;

        // Extract error entries with proper codes from poll data
        for entry in file_statuses {
            if entry.status == FileStatusKind::Error
                && !self.errors.seen_files.contains(&*entry.filename)
            {
                let code = entry.error_codes.as_ref().and_then(|c| c.first()).cloned();
                let msg = entry
                    .error
                    .clone()
                    .unwrap_or_else(|| "unknown error".into());
                self.errors.seen_files.insert(entry.filename.to_string());
                self.errors.entries.push(ErrorEntry {
                    filename: split_dir_file(&entry.filename).1.to_string(),
                    code,
                    message: msg,
                });
            }
        }

        // Clamp focus
        if !self.directories.groups.is_empty()
            && self.directories.focused_group >= self.directories.groups.len()
        {
            self.directories.focused_group = self.directories.groups.len() - 1;
        }
    }

    /// Apply one reducer message produced by the poll side of the TUI boundary.
    pub fn apply_update(&mut self, update: TuiUpdate) {
        match update {
            TuiUpdate::PollSnapshot {
                done,
                file_statuses,
            } => {
                self.update_from_poll(done, &file_statuses);
            }
            TuiUpdate::FileError { filename, message } => {
                self.add_error(&filename, &message, None);
            }
            TuiUpdate::HealthSnapshot(h) => {
                self.health = Some(h);
            }
            TuiUpdate::Finished => {
                self.progress.finished = true;
            }
            TuiUpdate::CancelledReceipt(receipt) => {
                self.progress.cancelled_receipt = Some(receipt);
            }
        }
    }

    /// Add an error entry, deduplicating by filename.
    pub fn add_error(&mut self, filename: &str, msg: &str, code: Option<&str>) {
        if self.errors.seen_files.contains(filename) {
            return;
        }
        self.errors.seen_files.insert(filename.to_string());
        self.errors.entries.push(ErrorEntry {
            filename: filename.to_string(),
            code: code.map(str::to_string),
            message: msg.to_string(),
        });
    }

    /// Scroll up within the focused group.
    pub fn scroll_up(&mut self) {
        self.directories.scroll_offset = self.directories.scroll_offset.saturating_sub(1);
    }

    /// Scroll down within the focused group.
    pub fn scroll_down(&mut self) {
        if let Some(group) = self.directories.groups.get(self.directories.focused_group) {
            let max = group.files.len().saturating_sub(1);
            if self.directories.scroll_offset < max {
                self.directories.scroll_offset += 1;
            }
        }
    }

    /// Cycle to the next directory group.
    pub fn cycle_group(&mut self) {
        if !self.directories.groups.is_empty() {
            self.directories.focused_group =
                (self.directories.focused_group + 1) % self.directories.groups.len();
            self.directories.scroll_offset = 0;
        }
    }

    /// Toggle the error panel expansion.
    pub fn toggle_errors(&mut self) {
        self.errors.expanded = !self.errors.expanded;
    }

    /// Toggle the worker/memory metrics rows.
    pub fn toggle_metrics(&mut self) {
        self.show_metrics = !self.show_metrics;
    }

    /// Return whether the job is locally marked as finished.
    pub fn is_finished(&self) -> bool {
        self.progress.finished
    }

    /// Return whether the cancel-confirm prompt is currently visible.
    pub fn cancel_confirm_active(&self) -> bool {
        self.interaction.cancel_confirm
    }

    /// Show the cancel-confirm prompt when the job is still active.
    pub fn request_cancel_confirmation(&mut self) {
        if !self.is_finished() {
            self.interaction.cancel_confirm = true;
        }
    }

    /// Return the first in-flight filename across all directory groups, or
    /// `None` if no file is currently processing. Used by the cancel handler
    /// to attribute the cancel to a specific file in the audit table.
    ///
    /// Most jobs have at most one in-flight file at a time (per-file
    /// dispatch with bounded parallelism = 1); if there are multiple, this
    /// returns whichever the directory iteration encounters first. Forensic
    /// readers can cross-reference against the per-file `attempts` table
    /// for the full picture.
    pub fn first_active_filename(&self) -> Option<String> {
        for group in &self.directories.groups {
            for file in &group.files {
                if file.status == FileStatusKind::Processing {
                    return Some(format!("{}/{}", group.dir, file.name));
                }
            }
        }
        None
    }

    /// Clear the cancel-confirm prompt.
    pub fn clear_cancel_confirmation(&mut self) {
        self.interaction.cancel_confirm = false;
    }

    /// Advance the spinner animation one frame.
    pub fn tick_spinner(&mut self) {
        self.directories.spinner_tick = self.directories.spinner_tick.wrapping_add(1);
    }
}

/// Split "dir/subdir/file.cha" into ("dir/subdir", "file.cha").
/// If no directory component, returns (".", filename).
fn split_dir_file(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(idx) => (&path[..idx], &path[idx + 1..]),
        None => (".", path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(filename: &str, status: FileStatusKind) -> FileStatusEntry {
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

    #[test]
    fn files_sorted_alphabetically_within_group() {
        let mut state = AppState::new(3, "align");
        // Deliberately out of order — server returns them in arbitrary order.
        let entries = vec![
            make_entry("PWA/ACWT10a.cha", FileStatusKind::Queued),
            make_entry("PWA/ACWT02a.cha", FileStatusKind::Processing),
            make_entry("PWA/ACWT05a.cha", FileStatusKind::Done),
        ];
        state.update_from_poll(1, &entries);

        let files = &state.directories.groups[0].files;
        assert_eq!(files[0].name, "ACWT02a.cha");
        assert_eq!(files[1].name, "ACWT05a.cha");
        assert_eq!(files[2].name, "ACWT10a.cha");
    }

    #[test]
    fn groups_files_by_directory() {
        let mut state = AppState::new(4, "morphotag");
        let entries = vec![
            make_entry("eng/a.cha", FileStatusKind::Done),
            make_entry("eng/b.cha", FileStatusKind::Processing),
            make_entry("spa/c.cha", FileStatusKind::Queued),
            make_entry("spa/d.cha", FileStatusKind::Error),
        ];
        state.update_from_poll(1, &entries);

        assert_eq!(state.directories.groups.len(), 2);
        assert_eq!(state.directories.groups[0].dir, "eng");
        assert_eq!(state.directories.groups[0].files.len(), 2);
        assert_eq!(state.directories.groups[0].done_count, 1);
        assert_eq!(state.directories.groups[0].active_count, 1);
        assert_eq!(state.directories.groups[1].dir, "spa");
        assert_eq!(state.directories.groups[1].error_count, 1);
    }

    #[test]
    fn group_counts_correct() {
        let mut state = AppState::new(3, "morphotag");
        let entries = vec![
            make_entry("d/a.cha", FileStatusKind::Done),
            make_entry("d/b.cha", FileStatusKind::Done),
            make_entry("d/c.cha", FileStatusKind::Error),
        ];
        state.update_from_poll(2, &entries);

        assert_eq!(state.directories.groups.len(), 1);
        assert_eq!(state.directories.groups[0].done_count, 2);
        assert_eq!(state.directories.groups[0].error_count, 1);
        assert_eq!(state.directories.groups[0].active_count, 0);
    }

    #[test]
    fn scroll_and_focus_preserved() {
        let mut state = AppState::new(4, "morphotag");
        let entries = vec![
            make_entry("a/x.cha", FileStatusKind::Queued),
            make_entry("b/y.cha", FileStatusKind::Queued),
        ];
        state.update_from_poll(0, &entries);
        state.directories.focused_group = 1;
        state.directories.scroll_offset = 0;

        // Update again — focus should be preserved
        state.update_from_poll(0, &entries);
        assert_eq!(state.directories.focused_group, 1);
    }

    #[test]
    fn focus_clamped_when_groups_shrink() {
        let mut state = AppState::new(2, "morphotag");
        state.directories.focused_group = 5;
        let entries = vec![make_entry("d/a.cha", FileStatusKind::Done)];
        state.update_from_poll(1, &entries);

        assert_eq!(state.directories.focused_group, 0);
    }

    #[test]
    fn split_dir_file_basic() {
        assert_eq!(
            split_dir_file("eng/Eng-NA/test.cha"),
            ("eng/Eng-NA", "test.cha")
        );
        assert_eq!(split_dir_file("test.cha"), (".", "test.cha"));
    }

    #[test]
    fn reducer_applies_poll_snapshot() {
        let mut state = AppState::new(1, "morphotag");
        state.apply_update(TuiUpdate::PollSnapshot {
            done: 1,
            file_statuses: vec![make_entry("eng/a.cha", FileStatusKind::Done)],
        });

        assert_eq!(state.progress.completed, 1);
        assert_eq!(state.directories.groups.len(), 1);
        assert_eq!(state.directories.groups[0].done_count, 1);
    }

    #[test]
    fn reducer_marks_finished() {
        let mut state = AppState::new(1, "morphotag");
        assert!(!state.is_finished());

        state.apply_update(TuiUpdate::Finished);
        assert!(state.is_finished());
    }

    /// `CancelledReceipt` is the durable record the user sees after a
    /// cancel — replaces the misleading "Done — N failed" suffix with
    /// "you pressed c-y from <host> at <time>" using the audit-table
    /// fields exposed via `JobInfo.last_cancelled_*`.
    #[test]
    fn reducer_records_cancellation_receipt() {
        let mut state = AppState::new(1, "morphotag");
        assert!(state.progress.cancelled_receipt.is_none());

        let receipt = CancelledReceipt {
            source: "tui".to_string(),
            host: Some("test-laptop".to_string()),
            reason: Some("user-pressed-cancel".to_string()),
            at_iso: "2026-04-26T14:34:13+00:00".to_string(),
        };
        state.apply_update(TuiUpdate::CancelledReceipt(receipt.clone()));

        assert_eq!(state.progress.cancelled_receipt, Some(receipt));
    }

    #[test]
    fn cycle_group_wraps() {
        let mut state = AppState::new(2, "morphotag");
        let entries = vec![
            make_entry("a/x.cha", FileStatusKind::Queued),
            make_entry("b/y.cha", FileStatusKind::Queued),
        ];
        state.update_from_poll(0, &entries);

        state.cycle_group();
        assert_eq!(state.directories.focused_group, 1);
        state.cycle_group();
        assert_eq!(state.directories.focused_group, 0);
    }

    #[test]
    fn progress_stage_propagated_from_poll() {
        let mut state = AppState::new(1, "align");
        let mut entry = make_entry("eng/a.cha", FileStatusKind::Processing);
        entry.progress_stage = Some(FileProgressStage::Aligning);
        state.update_from_poll(0, &[entry]);

        let file = &state.directories.groups[0].files[0];
        assert_eq!(file.progress_stage, Some(FileProgressStage::Aligning));
    }

    #[test]
    fn health_snapshot_stored_via_reducer() {
        let mut state = AppState::new(1, "morphotag");
        assert!(state.health.is_none());

        state.apply_update(TuiUpdate::HealthSnapshot(ServerHealth {
            live_workers: 3,
            live_worker_keys: vec!["infer:morphosyntax:eng".into()],
            system_memory_total_mb: MemoryMb(262144),
            system_memory_available_mb: MemoryMb(100000),
            system_memory_used_mb: MemoryMb(162144),
            memory_gate_threshold_mb: MemoryMb(2048),
            warmup_status: "complete".into(),
            host_memory_pressure: "healthy".into(),
            worker_crashes: 0,
            attempts_started: 5,
        }));

        let h = state.health.as_ref().unwrap();
        assert_eq!(h.live_workers, 3);
        assert_eq!(h.system_memory_total_mb, MemoryMb(262144));
    }

    #[test]
    fn toggle_metrics_flips_visibility() {
        let mut state = AppState::new(1, "morphotag");
        assert!(state.show_metrics); // default: shown
        state.toggle_metrics();
        assert!(!state.show_metrics);
        state.toggle_metrics();
        assert!(state.show_metrics);
    }

    #[test]
    fn error_codes_extracted_from_poll_entries() {
        let mut state = AppState::new(1, "morphotag");
        let mut entry = make_entry("eng/a.cha", FileStatusKind::Error);
        entry.error = Some("morph lookup failed".into());
        entry.error_codes = Some(vec!["E4012".into()]);
        state.update_from_poll(0, &[entry]);

        assert_eq!(state.errors.entries.len(), 1);
        assert_eq!(state.errors.entries[0].code.as_deref(), Some("E4012"));
        assert_eq!(state.errors.entries[0].message, "morph lookup failed");
    }

    #[test]
    fn error_entries_deduplicated_across_polls() {
        let mut state = AppState::new(1, "morphotag");
        let mut entry = make_entry("eng/a.cha", FileStatusKind::Error);
        entry.error = Some("fail".into());

        // Two poll ticks with the same error file
        state.update_from_poll(0, &[entry.clone()]);
        state.update_from_poll(0, &[entry]);

        assert_eq!(state.errors.entries.len(), 1);
    }

    #[test]
    fn queued_count_tracked_in_groups() {
        let mut state = AppState::new(3, "morphotag");
        let entries = vec![
            make_entry("d/a.cha", FileStatusKind::Done),
            make_entry("d/b.cha", FileStatusKind::Queued),
            make_entry("d/c.cha", FileStatusKind::Queued),
        ];
        state.update_from_poll(1, &entries);

        assert_eq!(state.directories.groups[0].queued_count, 2);
    }

    #[test]
    fn started_at_propagated_from_poll() {
        let mut state = AppState::new(1, "align");
        let mut entry = make_entry("eng/a.cha", FileStatusKind::Processing);
        entry.started_at = Some(crate::api::UnixTimestamp(1710000000.0));
        state.update_from_poll(0, &[entry]);

        let file = &state.directories.groups[0].files[0];
        assert_eq!(file.started_at, Some(1710000000.0));
    }
}
