//! Test fixtures for job lifecycle unit tests.
//!
//! Centralised so that every lifecycle test starts from the same
//! well-known in-memory state rather than re-inventing the fixture
//! in each test module.

use std::collections::{BTreeMap, HashMap};

use crate::api::{
    DisplayPath, JobStatus, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
    UnixTimestamp,
};
use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};
use crate::store::{
    FileStatus,
    job::{
        Job, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity,
        JobRuntimeControl, JobScheduleState, JobSourceContext,
    },
};

use tokio_util::sync::CancellationToken;

use super::types::JobLeaseState;

/// Build a minimal `Job` whose `execution.status = Running`.
///
/// Used by lifecycle unit tests that need an in-flight job to
/// transition via `interrupt_for_shutdown` and similar operations.
/// All fields not relevant to lifecycle are set to inert defaults;
/// the fixture contains one file (`job-file.cha`) in `FileStatusKind::Queued`
/// so that `pending_files()` / `all_terminal_files_failed()` have a
/// non-empty domain to act on.
pub(crate) fn running_job_fixture() -> Job {
    let mut file_statuses = HashMap::new();
    let filename = DisplayPath::from("job-file.cha");
    file_statuses.insert(
        String::from(filename.clone()),
        FileStatus::new(filename.clone()),
    );

    Job {
        identity: JobIdentity {
            job_id: "test-job-running".into(),
            correlation_id: "corr-test-running".into(),
        },
        dispatch: JobDispatchConfig {
            command: ReleasedCommand::Morphotag,
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            num_speakers: NumSpeakers(1),
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),
                ..Default::default()
            }),
            runtime_state: BTreeMap::new(),
            debug_traces: false,
        },
        source: JobSourceContext {
            submitted_by: "127.0.0.1".into(),
            submitted_by_name: String::new(),
            source_dir: Default::default(),
        },
        filesystem: JobFilesystemConfig {
            filenames: vec![filename],
            has_chat: vec![true],
            staging_dir: Default::default(),
            paths_mode: false,
            source_paths: Vec::new(),
            output_paths: Vec::new(),
            before_paths: Vec::new(),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
        },
        execution: JobExecutionState {
            status: JobStatus::Running,
            file_statuses,
            results: Vec::new(),
            error: None,
            completed_files: 0,
            batch_progress: None,
        },
        schedule: JobScheduleState {
            submitted_at: UnixTimestamp(1_000_000.0),
            completed_at: None,
            next_eligible_at: None,
            num_workers: None,
            lease: JobLeaseState {
                leased_by_node: None,
                expires_at: None,
                heartbeat_at: None,
            },
            last_cancel: None,
        },
        runtime: JobRuntimeControl {
            cancel_token: CancellationToken::new(),
            runner_active: true,
        },
        execution_plan: None,
    }
}

/// Build a `Job` with three named files in the `file_statuses` map,
/// all still `Queued`. Mirrors the 2026-05-11 morphotag job
/// (`150c824a-48e`) shape: 3 files submitted, none completed yet.
/// Tests then transition individual files via the standard
/// `mark_file_done` / `mark_file_error` methods.
pub(crate) fn three_file_job_fixture() -> Job {
    let mut job = running_job_fixture();
    let names = ["65-3.cha", "60home-3.cha", "65home-3.cha"];
    job.execution.file_statuses.clear();
    job.filesystem.filenames.clear();
    job.filesystem.has_chat.clear();
    for name in names {
        let path = DisplayPath::from(name);
        job.execution
            .file_statuses
            .insert(String::from(path.clone()), FileStatus::new(path.clone()));
        job.filesystem.filenames.push(path);
        job.filesystem.has_chat.push(true);
    }
    job
}
