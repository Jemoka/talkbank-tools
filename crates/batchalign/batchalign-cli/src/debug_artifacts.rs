//! Shared debug-artifact model for inspectable job runs.
//!
//! Direct and server-backed execution should share the shape of the debugging
//! handles they expose even if they do not share the same persistence or
//! transport layer.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::JobId;
use crate::store::JobDetail;

/// Stable debug handles for one completed or inspectable job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobDebugArtifacts {
    /// Stable job identifier.
    pub job_id: JobId,
    /// Host-local staging directory containing input/output/debug artifacts.
    pub staging_dir: PathBuf,
    /// Persisted trace file when trace capture was enabled and exported.
    pub trace_file: Option<PathBuf>,
    /// Bug-report identifiers referenced by file statuses.
    pub bug_report_ids: Vec<String>,
    /// Host-local bug-report files derived from [`Self::bug_report_ids`].
    pub bug_report_files: Vec<PathBuf>,
}

impl JobDebugArtifacts {
    /// Build one debug-artifact summary from a job detail snapshot.
    pub fn from_job_detail(
        job_id: JobId,
        detail: &JobDetail,
        bug_reports_dir: &Path,
        trace_file: Option<PathBuf>,
    ) -> Self {
        let bug_report_ids = detail
            .file_statuses
            .iter()
            .filter_map(|entry| entry.bug_report_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let bug_report_files = bug_report_ids
            .iter()
            .map(|id| bug_reports_dir.join(format!("{id}.json")))
            .collect();

        Self {
            job_id,
            staging_dir: detail.staging_dir.as_path().to_owned(),
            trace_file,
            bug_report_ids,
            bug_report_files,
        }
    }
}
