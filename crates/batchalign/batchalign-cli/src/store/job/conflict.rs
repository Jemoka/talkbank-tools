//! Job conflict detection.
//!
//! When a new job is submitted, its files are checked against all currently
//! active jobs from the same submitter.  Conflicts are keyed on
//! `(submitted_by, source_dir/filename)` so that the same client cannot
//! accidentally double-submit a file that is already being processed.

use std::collections::HashMap;

use crate::api::{DisplayPath, JobId, JobStatus, ReleasedCommand};

use super::Job;

/// Describes one file-level collision between an incoming job submission and an
/// existing active job.
///
/// Conflict detection is keyed on `(submitted_by, filename)`.  If the same
/// client tries to submit a file that is already being processed, one
/// `ConflictEntry` is produced per overlapping filename.  The entries are
/// returned in the 409 Conflict response so the client knows which files
/// collided and with which jobs.
#[derive(Debug)]
pub struct ConflictEntry {
    /// Basename of the conflicting file.
    pub filename: DisplayPath,
    /// Job ID of the existing active job that owns this file.
    pub job_id: JobId,
    /// Command of the existing active job.
    pub command: ReleasedCommand,
    /// Status of the existing active job.
    pub status: JobStatus,
}

/// Find file-level conflicts between an incoming job and all active jobs.
pub(crate) fn find_conflicts(jobs: &HashMap<JobId, Job>, incoming: &Job) -> Vec<ConflictEntry> {
    let incoming_keys: std::collections::HashSet<(String, String)> = incoming
        .filesystem
        .filenames
        .iter()
        .map(|fn_| {
            let path = if incoming.source.source_dir.is_empty() {
                String::from(fn_.clone())
            } else {
                format!("{}/{fn_}", incoming.source.source_dir)
            };
            (incoming.source.submitted_by.clone(), path)
        })
        .collect();

    let mut conflicts = Vec::new();
    for active in jobs.values() {
        if !active.execution.status.is_active() {
            continue;
        }
        for fn_ in &active.filesystem.filenames {
            let path = if active.source.source_dir.is_empty() {
                String::from(fn_.clone())
            } else {
                format!("{}/{fn_}", active.source.source_dir)
            };
            let key = (active.source.submitted_by.clone(), path);
            if incoming_keys.contains(&key) {
                conflicts.push(ConflictEntry {
                    filename: fn_.clone(),
                    job_id: active.identity.job_id.clone(),
                    command: active.dispatch.command,
                    status: active.execution.status,
                });
            }
        }
    }
    conflicts
}
