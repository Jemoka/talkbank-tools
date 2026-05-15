use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::api::{
    DisplayPath, JobId, JobStatus, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
    UnixTimestamp,
};
use crate::db::JobDB;
use crate::options::{CommandOptions, CommonOptions, OpensmileOptions};
use crate::scheduling::{AttemptOutcome, FailureCategory, WorkUnitKind};
use crate::store::{
    FileStatus, Job, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity,
    JobLeaseState, JobRuntimeControl, JobScheduleState, JobSourceContext, JobStore, PendingJobFile,
};
use crate::worker::InferTask;
use crate::ws::BROADCAST_CAPACITY;

use super::util::StoreRunnerEventSink;
use super::{
    command_requires_chat_infer, infer_task_for_command, record_preflight_media_failures,
    result_filename_for_command,
};

/// Build a minimal paths-mode media job for prevalidation tests.
fn make_media_job(job_id: &str, source_path: &str) -> Job {
    let filename = "missing.wav";
    let mut file_statuses = HashMap::new();
    file_statuses.insert(
        filename.to_string(),
        FileStatus::new(DisplayPath::from(filename)),
    );

    Job {
        identity: JobIdentity {
            job_id: JobId::from(job_id),
            correlation_id: format!("test-{job_id}").into(),
        },
        dispatch: JobDispatchConfig {
            command: ReleasedCommand::Opensmile,
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            num_speakers: NumSpeakers(1),
            options: CommandOptions::Opensmile(OpensmileOptions {
                common: CommonOptions::default(),
                feature_set: "eGeMAPSv02".into(),
            }),
            runtime_state: BTreeMap::new(),
            debug_traces: false,
        },
        source: JobSourceContext {
            submitted_by: "127.0.0.1".into(),
            submitted_by_name: "localhost".into(),
            source_dir: Default::default(),
        },
        filesystem: JobFilesystemConfig {
            filenames: vec![DisplayPath::from(filename)],
            has_chat: vec![false],
            staging_dir: Default::default(),
            paths_mode: true,
            source_paths: vec![batchalign_types::paths::ClientPath::new(source_path)],
            output_paths: vec![Default::default()],
            before_paths: Vec::new(),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
        },
        execution: JobExecutionState {
            status: JobStatus::Queued,
            file_statuses,
            results: Vec::new(),
            error: None,
            completed_files: 0,
            batch_progress: None,
        },
        schedule: JobScheduleState {
            submitted_at: UnixTimestamp(1.0),
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
            runner_active: false,
        },
        execution_plan: None,
    }
}

#[test]
fn infer_task_mapping_is_stable() {
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Morphotag),
        Some(InferTask::Morphosyntax)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Utseg),
        Some(InferTask::Utseg)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Translate),
        Some(InferTask::Translate)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Coref),
        Some(InferTask::Coref)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Align),
        Some(InferTask::Fa)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Transcribe),
        Some(InferTask::Asr)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Compare),
        Some(InferTask::Morphosyntax)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Opensmile),
        Some(InferTask::Opensmile)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Avqi),
        Some(InferTask::Avqi)
    );
    assert_eq!(
        infer_task_for_command(ReleasedCommand::Benchmark),
        Some(InferTask::Asr)
    );
}

#[test]
fn chat_backed_commands_require_chat_infer() {
    for cmd in [
        ReleasedCommand::Morphotag,
        ReleasedCommand::Utseg,
        ReleasedCommand::Translate,
        ReleasedCommand::Coref,
    ] {
        assert!(command_requires_chat_infer(cmd));
    }
}

#[test]
fn align_requires_chat_infer() {
    assert!(command_requires_chat_infer(ReleasedCommand::Align));
}

#[test]
fn audio_or_composed_commands_do_not_require_chat_infer() {
    assert!(!command_requires_chat_infer(ReleasedCommand::Opensmile));
    assert!(!command_requires_chat_infer(ReleasedCommand::Avqi));
    assert!(!command_requires_chat_infer(ReleasedCommand::Transcribe));
    assert!(!command_requires_chat_infer(ReleasedCommand::Benchmark));
}

#[test]
fn transcribe_result_filename_preserves_relative_path() {
    assert_eq!(
        result_filename_for_command(ReleasedCommand::Transcribe, "sub/nested.wav"),
        "sub/nested.cha"
    );
    assert_eq!(
        result_filename_for_command(ReleasedCommand::TranscribeS, "nested.mp3"),
        "nested.cha"
    );
}

#[test]
fn non_transcribe_result_filename_is_unchanged() {
    assert_eq!(
        result_filename_for_command(ReleasedCommand::Morphotag, "sub/nested.cha"),
        "sub/nested.cha"
    );
}

/// Preflight media validation should still leave a durable setup attempt so
/// the file's failure appears in the attempt log instead of only in file
/// status.
#[tokio::test]
async fn preflight_media_failure_records_setup_attempt() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let missing_path = tempdir.path().join("missing.wav");
    let db = Arc::new(JobDB::open(Some(tempdir.path())).await.expect("open db"));
    let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(
        crate::config::ServerConfig::default(),
        Some(db.clone()),
        tx,
    ));
    store
        .submit(make_media_job(
            "job-media-preflight",
            &missing_path.display().to_string(),
        ))
        .await
        .expect("submit job");

    let file_list = vec![PendingJobFile {
        file_index: 0,
        filename: DisplayPath::from("missing.wav"),
        has_chat: false,
    }];
    let failures = HashMap::from([(0usize, String::from("Media file not found"))]);
    let sink = StoreRunnerEventSink::wrap(store.clone());

    let failed_indices = record_preflight_media_failures(
        sink.as_ref(),
        &JobId::from("job-media-preflight"),
        &file_list,
        &failures,
    )
    .await;

    assert_eq!(failed_indices, HashSet::from([0usize]));

    let attempts = db
        .load_attempts_for_job("job-media-preflight")
        .await
        .expect("load attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].work_unit_kind, WorkUnitKind::FileSetup);
    assert_eq!(attempts[0].outcome, AttemptOutcome::Failed);
    assert_eq!(
        attempts[0].failure_category,
        Some(FailureCategory::Validation)
    );

    let detail = store
        .get_job_detail(&JobId::from("job-media-preflight"))
        .await
        .expect("job detail");
    assert_eq!(detail.file_statuses.len(), 1);
    assert_eq!(
        detail.file_statuses[0].status,
        crate::api::FileStatusKind::Error
    );
}
