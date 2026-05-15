//! Tests for file status tracking, supervision, and progress forwarding.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::api::{
    ContentType, DisplayPath, FileProgressStage, FileStatusKind, JobId, JobStatus, LanguageCode3,
    LanguageSpec, NumSpeakers, ReleasedCommand, UnixTimestamp,
};
use crate::db::JobDB;
use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};
use crate::scheduling::{AttemptOutcome, FailureCategory, RetryDisposition, WorkUnitKind};
use crate::store::unix_now;
use crate::store::{
    CompletedFileOutput, FileStatus, Job, JobDispatchConfig, JobExecutionState,
    JobFilesystemConfig, JobIdentity, JobLeaseState, JobRuntimeControl, JobScheduleState,
    JobSourceContext,
};
use crate::ws::BROADCAST_CAPACITY;

use super::tracker::mark_file_processing;
use super::*;
use crate::store::JobStore;

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedProgress {
    job_id: JobId,
    filename: String,
    stage: FileStage,
    current: Option<i64>,
    total: Option<i64>,
}

#[derive(Default)]
struct RecordingSink {
    progress: Mutex<Vec<RecordedProgress>>,
}

#[async_trait]
impl RunnerEventSink for RecordingSink {
    async fn mark_file_processing(
        &self,
        _job_id: &JobId,
        _filename: &str,
        _started_at: UnixTimestamp,
    ) {
    }

    async fn mark_file_done(
        &self,
        _job_id: &JobId,
        _filename: &str,
        _finished_at: UnixTimestamp,
        _result: Option<CompletedFileOutput>,
    ) {
    }

    async fn mark_file_error(
        &self,
        _job_id: &JobId,
        _filename: &str,
        _error: &str,
        _category: FailureCategory,
        _finished_at: UnixTimestamp,
    ) {
    }

    async fn start_file_attempt(
        &self,
        _job_id: &JobId,
        _filename: &str,
        _work_unit_kind: WorkUnitKind,
        _started_at: UnixTimestamp,
    ) {
    }

    async fn finish_file_attempt(
        &self,
        _job_id: &JobId,
        _filename: &str,
        _outcome: AttemptOutcome,
        _failure_category: Option<FailureCategory>,
        _disposition: RetryDisposition,
        _finished_at: UnixTimestamp,
    ) {
    }

    async fn mark_file_retry_pending(
        &self,
        _job_id: &JobId,
        _filename: &str,
        _retry_at: UnixTimestamp,
        _category: FailureCategory,
        _message: &str,
        _finished_at: UnixTimestamp,
    ) {
    }

    async fn clear_file_retry_state(&self, _job_id: &JobId, _filename: &str) {}

    async fn set_file_progress(
        &self,
        job_id: &JobId,
        filename: &str,
        stage: FileStage,
        current: Option<i64>,
        total: Option<i64>,
    ) {
        self.progress
            .lock()
            .expect("progress lock")
            .push(RecordedProgress {
                job_id: job_id.clone(),
                filename: filename.to_string(),
                stage,
                current,
                total,
            });
    }

    async fn unfinished_files(&self, _job_id: &JobId) -> Vec<DisplayPath> {
        Vec::new()
    }

    async fn file_status_label(&self, _job_id: &JobId, _filename: &str) -> Option<String> {
        None
    }

    async fn bump_forced_terminal_errors(&self, _count: usize) {}

    async fn fail_job(&self, _job_id: &JobId, _error: &str, _failed_at: UnixTimestamp) {}

    async fn mark_job_running(&self, _job_id: &JobId) {}

    async fn record_job_worker_count(&self, _job_id: &JobId, _worker_count: usize) {}

    async fn requeue_job_after_memory_gate(&self, _job_id: &JobId, _retry_at: UnixTimestamp) {}

    async fn bump_deferred_work_units(&self) {}

    async fn bump_memory_gate_aborts(&self) {}

    async fn finalize_job(
        &self,
        _job_id: &JobId,
        _final_status: JobStatus,
        _completed_at: UnixTimestamp,
    ) {
    }
}

fn test_config() -> crate::config::ServerConfig {
    crate::config::ServerConfig {
        max_concurrent_jobs: Some(2),
        ..Default::default()
    }
}

fn make_job(id: &str) -> Job {
    let mut file_statuses = HashMap::new();
    file_statuses.insert(
        "a.cha".to_string(),
        FileStatus::new(DisplayPath::from("a.cha")),
    );

    Job {
        identity: JobIdentity {
            job_id: id.into(),
            correlation_id: format!("test-{id}").into(),
        },
        dispatch: JobDispatchConfig {
            command: ReleasedCommand::Morphotag,
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            num_speakers: NumSpeakers(1),
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
            runtime_state: std::collections::BTreeMap::new(),
            debug_traces: false,
        },
        source: JobSourceContext {
            submitted_by: "127.0.0.1".into(),
            submitted_by_name: String::new(),
            source_dir: Default::default(),
        },
        filesystem: JobFilesystemConfig {
            filenames: vec![DisplayPath::from("a.cha")],
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
            status: JobStatus::Queued,
            file_statuses,
            results: Vec::new(),
            error: None,
            completed_files: 0,
            batch_progress: None,
        },
        schedule: JobScheduleState {
            submitted_at: unix_now(),
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

#[tokio::test]
async fn progress_forwarder_routes_updates_through_sink_boundary() {
    let sink = Arc::new(RecordingSink::default());
    let job_id = JobId::from("job-progress");
    let tx = spawn_progress_forwarder(sink.clone(), job_id.clone(), "a.cha".to_string());

    tx.send(ProgressUpdate::new(FileStage::Writing, Some(1), Some(3)))
        .expect("send progress update");
    drop(tx);

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !sink.progress.lock().expect("progress lock").is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("progress forwarder should flush");

    let progress = sink.progress.lock().expect("progress lock");
    assert_eq!(
        progress.as_slice(),
        &[RecordedProgress {
            job_id,
            filename: "a.cha".to_string(),
            stage: FileStage::Writing,
            current: Some(1),
            total: Some(3),
        }]
    );
}

#[tokio::test]
async fn supervised_task_marks_non_terminal_exit_as_error() {
    let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(test_config(), None, tx));
    let sink = StoreRunnerEventSink::wrap(store.clone());
    let job_id = JobId::from("job-1");
    store.submit(make_job("job-1")).await.unwrap();

    mark_file_processing(sink.as_ref(), &job_id, "a.cha", unix_now()).await;

    let tasks = vec![spawn_supervised_file_task(
        DisplayPath::from("a.cha"),
        "test file task",
        async { FileTaskOutcome::MissingTerminalState },
    )];

    let abnormal =
        drain_supervised_file_tasks(sink.as_ref(), &job_id, &CancellationToken::new(), tasks).await;
    assert_eq!(abnormal, 1);

    let detail = store.get_job_detail(&job_id).await.unwrap();
    let file = detail
        .file_statuses
        .into_iter()
        .find(|entry| entry.filename == "a.cha")
        .unwrap();
    assert_eq!(file.status, FileStatusKind::Error);
    assert!(
        file.error
            .as_deref()
            .is_some_and(|msg| msg.contains("exited without recording a terminal file state"))
    );
}

#[tokio::test]
async fn supervised_task_marks_panic_as_error() {
    let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(test_config(), None, tx));
    let sink = StoreRunnerEventSink::wrap(store.clone());
    let job_id = JobId::from("job-2");
    store.submit(make_job("job-2")).await.unwrap();

    mark_file_processing(sink.as_ref(), &job_id, "a.cha", unix_now()).await;

    let tasks = vec![spawn_supervised_file_task(
        DisplayPath::from("a.cha"),
        "panic file task",
        async {
            panic!("boom");
        },
    )];

    let abnormal =
        drain_supervised_file_tasks(sink.as_ref(), &job_id, &CancellationToken::new(), tasks).await;
    assert_eq!(abnormal, 1);

    let detail = store.get_job_detail(&job_id).await.unwrap();
    let file = detail
        .file_statuses
        .into_iter()
        .find(|entry| entry.filename == "a.cha")
        .unwrap();
    assert_eq!(file.status, FileStatusKind::Error);
    assert!(
        file.error
            .as_deref()
            .is_some_and(|msg| msg.contains("panicked before recording a terminal file state"))
    );
}

#[tokio::test]
async fn file_run_tracker_retries_then_completes_cleanly() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db = Arc::new(JobDB::open(Some(tempdir.path())).await.expect("open db"));
    let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(test_config(), Some(db.clone()), tx));
    let sink = StoreRunnerEventSink::wrap(store.clone());
    let job_id = JobId::from("job-tracker");
    store.submit(make_job("job-tracker")).await.unwrap();

    let lifecycle = FileRunTracker::new(sink.as_ref(), &job_id, "a.cha");
    let started_at = unix_now();
    lifecycle
        .begin_first_attempt(WorkUnitKind::FileProcess, started_at, FileStage::Reading)
        .await;

    let retry_finished_at = unix_now();
    let retry_at = crate::store::unix_now();
    lifecycle
        .retry(
            retry_at,
            FailureCategory::ProviderTransient,
            "temporary failure",
            retry_finished_at,
        )
        .await;

    let restarted_at = unix_now();
    lifecycle
        .restart_attempt(
            WorkUnitKind::FileProcess,
            restarted_at,
            FileStage::Processing,
        )
        .await;

    let finished_at = unix_now();
    lifecycle
        .complete_with_result(DisplayPath::from("a.ana"), ContentType::Chat, finished_at)
        .await;

    let detail = store.get_job_detail(&job_id).await.expect("job detail");
    let file = detail
        .file_statuses
        .into_iter()
        .find(|entry| entry.filename == "a.cha")
        .expect("tracked file");
    assert_eq!(file.status, FileStatusKind::Done);
    assert!(file.next_eligible_at.is_none());
    assert!(file.error.is_none());

    let attempts = db
        .load_attempts_for_job("job-tracker")
        .await
        .expect("load attempts");
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].outcome, AttemptOutcome::RetryableFailure);
    assert_eq!(attempts[0].disposition, RetryDisposition::Retry);
    assert_eq!(attempts[1].outcome, AttemptOutcome::Succeeded);
    assert_eq!(attempts[1].disposition, RetryDisposition::Succeed);
}

#[tokio::test]
async fn file_run_tracker_records_setup_failure() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let db = Arc::new(JobDB::open(Some(tempdir.path())).await.expect("open db"));
    let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
    let store = Arc::new(JobStore::new(test_config(), Some(db.clone()), tx));
    let sink = StoreRunnerEventSink::wrap(store.clone());
    let job_id = JobId::from("job-setup-failure");
    store.submit(make_job("job-setup-failure")).await.unwrap();

    let lifecycle = FileRunTracker::new(sink.as_ref(), &job_id, "a.cha");
    let started_at = unix_now();
    let finished_at = unix_now();
    lifecycle
        .record_setup_failure(
            started_at,
            "media preflight failed",
            FailureCategory::Validation,
            finished_at,
        )
        .await;

    let detail = store.get_job_detail(&job_id).await.expect("job detail");
    let file = detail
        .file_statuses
        .into_iter()
        .find(|entry| entry.filename == "a.cha")
        .expect("tracked file");
    assert_eq!(file.status, FileStatusKind::Error);
    assert_eq!(file.error.as_deref(), Some("media preflight failed"));

    let attempts = db
        .load_attempts_for_job("job-setup-failure")
        .await
        .expect("load attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].work_unit_kind, WorkUnitKind::FileSetup);
    assert_eq!(attempts[0].outcome, AttemptOutcome::Failed);
    assert_eq!(
        attempts[0].failure_category,
        Some(FailureCategory::Validation)
    );
}

#[test]
fn file_stage_for_batch_command_is_stable() {
    assert_eq!(
        FileStage::for_batch_command(ReleasedCommand::Morphotag),
        FileStage::Analyzing
    );
    assert_eq!(
        FileStage::for_batch_command(ReleasedCommand::Utseg),
        FileStage::Segmenting
    );
    assert_eq!(
        FileStage::for_batch_command(ReleasedCommand::Translate),
        FileStage::Translating
    );
    assert_eq!(
        FileStage::for_batch_command(ReleasedCommand::Coref),
        FileStage::ResolvingCoreference
    );
    assert_eq!(
        FileStage::for_batch_command(ReleasedCommand::Compare),
        FileStage::Comparing
    );
    assert_eq!(
        FileStage::for_batch_command(ReleasedCommand::Align),
        FileStage::Processing
    );
    assert_eq!(FileStage::Writing.api_stage().label(), "Writing");
    assert_eq!(
        FileStage::CheckingCache.api_stage().label(),
        "Checking cache"
    );
    assert_eq!(
        FileStage::PostProcessing.api_stage().label(),
        "Post-processing"
    );
    assert_eq!(FileStage::Aligning.api_stage(), FileProgressStage::Aligning);
    assert_eq!(
        FileStage::AnalyzingMorphosyntax.api_stage(),
        FileProgressStage::AnalyzingMorphosyntax
    );
    assert_eq!(
        FileStage::AnalyzingMorphosyntax.api_stage().label(),
        "Analyzing morphosyntax"
    );
}
