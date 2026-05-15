//! API response projections and runner-facing snapshots.
//!
//! These methods convert a `Job` into the various read-only views consumed by
//! HTTP handlers (`JobInfo`, `JobListItem`) and the job runner
//! (`RunnerJobSnapshot`).  They also produce the `pending_files()` list that
//! drives file-level dispatch.

use crate::api::{DurationSeconds, FileStatusEntry, FileStatusKind, JobInfo, JobListItem};
use crate::store::ts_iso;

use super::Job;
use super::types::{
    PendingJobFile, RunnerDispatchConfig, RunnerFilesystemConfig, RunnerJobIdentity,
    RunnerJobSnapshot,
};

impl Job {
    /// Return the stable job identifier.
    pub fn job_id(&self) -> &crate::api::JobId {
        &self.identity.job_id
    }

    /// Return the total number of logical files in the job.
    pub fn total_files(&self) -> usize {
        self.filesystem.filenames.len()
    }

    /// Convert to the API `JobInfo` response.
    pub fn to_info(&self) -> JobInfo {
        let file_statuses: Vec<FileStatusEntry> = self
            .execution
            .file_statuses
            .values()
            .map(|fs| fs.to_entry())
            .collect();
        let duration_s = self
            .schedule
            .completed_at
            .map(|c| DurationSeconds(c.0 - self.schedule.submitted_at.0));

        JobInfo {
            job_id: self.identity.job_id.clone(),
            status: self.execution.status,
            command: self.dispatch.command,
            options: self.dispatch.options.clone(),
            lang: self.dispatch.lang.clone(),
            source_dir: self.source.source_dir.as_str().to_owned(),
            total_files: self.total_files() as i64,
            completed_files: self.execution.completed_files,
            current_file: None,
            error: self.execution.error.clone(),
            file_statuses,
            submitted_at: Some(ts_iso(self.schedule.submitted_at)),
            submitted_by: if self.source.submitted_by.is_empty() {
                None
            } else {
                Some(self.source.submitted_by.clone())
            },
            submitted_by_name: if self.source.submitted_by_name.is_empty() {
                None
            } else {
                Some(self.source.submitted_by_name.clone())
            },
            completed_at: self.schedule.completed_at.map(ts_iso),
            duration_s,
            next_eligible_at: self.schedule.next_eligible_at,
            num_workers: self.schedule.num_workers,
            active_lease: self.active_lease(),
            batch_progress: self.execution.batch_progress.clone(),
            control_plane: None,
            execution_plan: self.execution_plan.clone(),
            last_cancelled_at: self.schedule.last_cancel.as_ref().map(|c| c.at),
            last_cancelled_source: self.schedule.last_cancel.as_ref().map(|c| c.source.clone()),
            last_cancelled_host: self
                .schedule
                .last_cancel
                .as_ref()
                .and_then(|c| c.host.clone()),
            last_cancelled_reason: self
                .schedule
                .last_cancel
                .as_ref()
                .and_then(|c| c.reason.clone()),
        }
    }

    /// Convert to the API `JobListItem` summary.
    pub fn to_list_item(&self) -> JobListItem {
        let error_files = self
            .execution
            .file_statuses
            .values()
            .filter(|fs| fs.status == FileStatusKind::Error)
            .count() as i64;
        let duration_s = self
            .schedule
            .completed_at
            .map(|c| DurationSeconds(c.0 - self.schedule.submitted_at.0));

        JobListItem {
            job_id: self.identity.job_id.clone(),
            status: self.execution.status,
            command: self.dispatch.command,
            lang: self.dispatch.lang.clone(),
            source_dir: self.source.source_dir.as_str().to_owned(),
            total_files: self.total_files() as i64,
            completed_files: self.execution.completed_files,
            error_files,
            submitted_at: Some(ts_iso(self.schedule.submitted_at)),
            submitted_by: if self.source.submitted_by.is_empty() {
                None
            } else {
                Some(self.source.submitted_by.clone())
            },
            submitted_by_name: if self.source.submitted_by_name.is_empty() {
                None
            } else {
                Some(self.source.submitted_by_name.clone())
            },
            completed_at: self.schedule.completed_at.map(ts_iso),
            duration_s,
            next_eligible_at: self.schedule.next_eligible_at,
            num_workers: self.schedule.num_workers,
            active_lease: self.active_lease(),
            control_plane: None,
        }
    }

    /// Return the files that have not yet reached a terminal state.
    pub fn pending_files(&self) -> Vec<PendingJobFile> {
        self.filesystem
            .filenames
            .iter()
            .enumerate()
            .zip(self.filesystem.has_chat.iter().copied())
            .filter_map(|((file_index, filename), has_chat)| {
                let already_done = self
                    .execution
                    .file_statuses
                    .get(&**filename)
                    .map(|status| status.status.is_terminal())
                    .unwrap_or(false);
                if already_done {
                    None
                } else {
                    Some(PendingJobFile {
                        file_index,
                        filename: filename.clone(),
                        has_chat,
                    })
                }
            })
            .collect()
    }

    /// Create the immutable runner-facing snapshot for this job.
    pub fn to_runner_snapshot(&self) -> RunnerJobSnapshot {
        RunnerJobSnapshot {
            identity: RunnerJobIdentity {
                job_id: self.identity.job_id.clone(),
                correlation_id: self.identity.correlation_id.clone(),
            },
            dispatch: RunnerDispatchConfig {
                command: self.dispatch.command,
                lang: self.dispatch.lang.clone(),
                num_speakers: self.dispatch.num_speakers,
                options: self.dispatch.options.clone(),
                runtime_state: self.dispatch.runtime_state.clone(),
                debug_traces: self.dispatch.debug_traces,
            },
            filesystem: RunnerFilesystemConfig {
                paths_mode: self.filesystem.paths_mode,
                source_paths: self.filesystem.source_paths.clone(),
                output_paths: self.filesystem.output_paths.clone(),
                before_paths: self.filesystem.before_paths.clone(),
                staging_dir: self.filesystem.staging_dir.clone(),
                media_mapping: self.filesystem.media_mapping.clone(),
                media_subdir: self.filesystem.media_subdir.clone(),
                source_dir: self.source.source_dir.clone(),
            },
            cancel_token: self.runtime.cancel_token.clone(),
            pending_files: self.pending_files(),
        }
    }
}
