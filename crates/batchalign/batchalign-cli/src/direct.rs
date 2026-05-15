//! Direct local execution host layered over the shared execution engine.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::warn;

use crate::api::{CorrelationId, JobId, JobInfo, JobSubmission};
use crate::cache::UtteranceCache;
use crate::config::{RuntimeLayout, ServerConfig};
use crate::debug_artifacts::JobDebugArtifacts;
use crate::error::ServerError;
use crate::runner::{DirectExecutionHost, run_direct_job};
use crate::store::{JobDetail, JobStore};
use crate::submission::{SubmissionContext, materialize_submission_job};
use crate::worker_setup::PreparedWorkers;
use crate::ws::BROADCAST_CAPACITY;

const DEBUG_ARTIFACTS_FILE: &str = "debug-artifacts.json";
const DEBUG_TRACES_FILE: &str = "debug-traces.json";

/// Final direct-execution projection returned to callers after one inline run.
pub struct DirectRunOutcome {
    /// Final lifecycle snapshot for the completed job.
    pub info: JobInfo,
    /// Detailed result projection for download/output handling.
    pub detail: JobDetail,
}

/// Host for one-shot local execution over the shared engine.
#[derive(Clone)]
pub struct DirectHost {
    store: Arc<JobStore>,
    runner: DirectExecutionHost,
    jobs_dir: PathBuf,
    bug_reports_dir: PathBuf,
    capabilities: Vec<String>,
}

impl DirectHost {
    fn ensure_supported_command(
        &self,
        command: crate::api::ReleasedCommand,
    ) -> Result<(), ServerError> {
        if self
            .capabilities
            .iter()
            .any(|capability| capability == command.as_ref())
        {
            return Ok(());
        }

        let supported = self
            .capabilities
            .iter()
            .filter(|capability| capability.as_str() != "test-echo")
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        Err(ServerError::UnknownCommand(format!(
            "Unknown command: {command}. Valid commands: {:?}",
            supported
        )))
    }

    /// Build one direct-execution host from a prepared worker subsystem.
    ///
    /// The host owns an in-memory [`JobStore`] and runs jobs inline with no
    /// queue, HTTP transport, registry discovery, or daemon lifecycle layer.
    pub async fn new(
        config: ServerConfig,
        layout: RuntimeLayout,
        jobs_dir: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
        workers: &PreparedWorkers,
    ) -> Result<Self, ServerError> {
        let jobs_dir = jobs_dir.unwrap_or_else(|| layout.jobs_dir());
        let bug_reports_dir = layout.bug_reports_dir();
        tokio::fs::create_dir_all(&jobs_dir).await?;
        tokio::fs::create_dir_all(&bug_reports_dir).await?;

        let cache = Arc::new(
            UtteranceCache::tiered(cache_dir, None)
                .await
                .map_err(|error| ServerError::Validation(format!("cache init failed: {error}")))?,
        );
        let execution_runtime = workers.resolve_execution_runtime(cache)?;
        let capabilities = execution_runtime.capability_snapshot.capabilities.clone();
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let store = Arc::new(JobStore::new(config, None, tx));
        let runner = DirectExecutionHost::new(store.clone(), execution_runtime.engine);

        Ok(Self {
            store,
            runner,
            jobs_dir,
            bug_reports_dir,
            capabilities,
        })
    }

    /// Return the released command surface available to this host.
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// Materialize and submit one direct job without running it yet.
    pub async fn submit_submission(&self, submission: JobSubmission) -> Result<JobId, ServerError> {
        self.ensure_supported_command(submission.command)?;
        let job_id = uuid::Uuid::new_v4().to_string()[..12].to_string();
        let correlation_id = CorrelationId::from(job_id.clone());
        let job = materialize_submission_job(
            &submission,
            &SubmissionContext {
                job_id: JobId::from(job_id.clone()),
                correlation_id,
                jobs_dir: self.jobs_dir.clone(),
                submitted_by: "127.0.0.1".into(),
                submitted_by_name: "direct-cli".into(),
            },
        )
        .await?;
        let job_id = job.identity.job_id.clone();

        self.store.submit(job).await?;
        Ok(job_id)
    }

    /// Run one previously submitted direct job inline.
    pub async fn run_job(&self, job_id: &JobId) -> Result<(), ServerError> {
        let result = run_direct_job(job_id, &self.runner).await;
        if let Err(error) = self.job_debug_artifacts(job_id).await {
            warn!(
                job_id = %job_id,
                error = %error,
                "Failed to persist direct debug artifacts"
            );
        }
        result
    }

    /// Fetch the current lifecycle snapshot for one direct job.
    pub async fn job_info(&self, job_id: &JobId) -> Result<JobInfo, ServerError> {
        self.store
            .get(job_id)
            .await
            .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))
    }

    /// Fetch the final result/detail projection for one direct job.
    pub async fn job_detail(&self, job_id: &JobId) -> Result<JobDetail, ServerError> {
        self.store
            .get_job_detail(job_id)
            .await
            .ok_or_else(|| ServerError::JobNotFound(job_id.clone()))
    }

    /// Persist and return machine-readable debug handles for one direct job.
    pub async fn job_debug_artifacts(
        &self,
        job_id: &JobId,
    ) -> Result<JobDebugArtifacts, ServerError> {
        let detail = self.job_detail(job_id).await?;
        let trace_file = self
            .persist_debug_traces(job_id, detail.staging_dir.as_ref())
            .await?;
        let artifacts = JobDebugArtifacts::from_job_detail(
            job_id.clone(),
            &detail,
            &self.bug_reports_dir,
            trace_file,
        );
        self.persist_debug_artifacts(&artifacts).await?;
        Ok(artifacts)
    }

    /// Run one submission inline and return its final projections.
    pub async fn run_submission(
        &self,
        submission: JobSubmission,
    ) -> Result<DirectRunOutcome, ServerError> {
        let job_id = self.submit_submission(submission).await?;
        self.run_job(&job_id).await?;
        let info = self.job_info(&job_id).await?;
        let detail = self.job_detail(&job_id).await?;
        Ok(DirectRunOutcome { info, detail })
    }

    async fn persist_debug_traces(
        &self,
        job_id: &JobId,
        staging_dir: &std::path::Path,
    ) -> Result<Option<PathBuf>, ServerError> {
        let Some(traces) = self.store.trace_store().get(job_id).await else {
            return Ok(None);
        };
        let trace_path = staging_dir.join(DEBUG_TRACES_FILE);
        let payload = serde_json::to_vec_pretty(&*traces).map_err(|error| {
            ServerError::Validation(format!(
                "serializing direct debug traces for {job_id}: {error}"
            ))
        })?;
        tokio::fs::write(&trace_path, payload)
            .await
            .map_err(|error| {
                ServerError::Io(std::io::Error::new(
                    error.kind(),
                    format!(
                        "persisting direct debug traces to {}: {error}",
                        trace_path.display()
                    ),
                ))
            })?;
        Ok(Some(trace_path))
    }

    async fn persist_debug_artifacts(
        &self,
        artifacts: &JobDebugArtifacts,
    ) -> Result<(), ServerError> {
        let path = artifacts.staging_dir.join(DEBUG_ARTIFACTS_FILE);
        let payload = serde_json::to_vec_pretty(artifacts).map_err(|error| {
            ServerError::Validation(format!(
                "serializing direct debug artifacts for {}: {error}",
                artifacts.job_id
            ))
        })?;
        tokio::fs::write(&path, payload).await.map_err(|error| {
            ServerError::Io(std::io::Error::new(
                error.kind(),
                format!(
                    "persisting direct debug artifacts to {}: {error}",
                    path.display()
                ),
            ))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DirectHost;
    use crate::api::{
        JobStatus, JobSubmission, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
    };
    use crate::options::{AsrEngineName, CommandOptions, CommonOptions, TranscribeOptions};
    use crate::types::traces::JobTraces;
    use crate::worker::pool::PoolConfig;
    use crate::worker_setup::prepare_direct_workers;
    use crate::{config::RuntimeLayout, config::ServerConfig};

    fn transcribe_submission(
        source_path: &std::path::Path,
        output_path: &std::path::Path,
    ) -> JobSubmission {
        JobSubmission {
            command: ReleasedCommand::Transcribe,
            lang: LanguageSpec::Resolved(LanguageCode3::eng()),
            num_speakers: NumSpeakers(1),
            files: Vec::new(),
            media_files: Vec::new(),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: source_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy()
                .into_owned()
                .into(),
            options: CommandOptions::Transcribe(TranscribeOptions {
                common: CommonOptions::default(),
                asr_engine: AsrEngineName::RevAi,
                diarize: false,
                wor: Default::default(),
                merge_abbrev: false.into(),
                utseg_fallback: false.into(),
                batch_size: 1,
            }),
            paths_mode: true,
            source_paths: vec![source_path.to_string_lossy().into_owned().into()],
            output_paths: vec![output_path.to_string_lossy().into_owned().into()],
            display_names: Vec::new(),
            debug_traces: false,
            before_paths: Vec::new(),
        }
    }

    #[tokio::test]
    async fn direct_host_runs_paths_mode_submission_inline() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        std::fs::create_dir_all(layout.state_dir()).expect("create state dir");

        let input_dir = tempdir.path().join("input");
        let output_dir = tempdir.path().join("output");
        std::fs::create_dir_all(&input_dir).expect("create input dir");
        std::fs::create_dir_all(&output_dir).expect("create output dir");

        let source_path = input_dir.join("sample.wav");
        let output_path = output_dir.join("sample.cha");
        let content = "@UTF8\n@Begin\n@End\n";
        tokio::fs::write(&source_path, b"RIFF")
            .await
            .expect("write input");

        let config = ServerConfig::default();
        let workers = prepare_direct_workers(
            &config,
            PoolConfig {
                test_echo: true,
                ..Default::default()
            },
        )
        .await
        .expect("prepare workers");
        let host = DirectHost::new(
            config,
            layout,
            None,
            Some(tempdir.path().join("cache")),
            &workers,
        )
        .await
        .expect("create direct host");

        let outcome = host
            .run_submission(transcribe_submission(&source_path, &output_path))
            .await
            .expect("run submission");

        assert_eq!(
            outcome.info.status,
            JobStatus::Completed,
            "job error: {:?}, file statuses: {:?}",
            outcome.info.error,
            outcome.info.file_statuses
        );
        assert!(outcome.detail.paths_mode);
        assert_eq!(outcome.detail.results.len(), 1);
        assert_eq!(
            tokio::fs::read_to_string(&output_path)
                .await
                .expect("read output"),
            content
        );
    }

    #[tokio::test]
    async fn direct_host_supports_submit_inspect_and_run_lifecycle() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        std::fs::create_dir_all(layout.state_dir()).expect("create state dir");

        let input_dir = tempdir.path().join("input");
        let output_dir = tempdir.path().join("output");
        std::fs::create_dir_all(&input_dir).expect("create input dir");
        std::fs::create_dir_all(&output_dir).expect("create output dir");

        let source_path = input_dir.join("sample.wav");
        let output_path = output_dir.join("sample.cha");
        tokio::fs::write(&source_path, b"RIFF")
            .await
            .expect("write input");

        let config = ServerConfig::default();
        let workers = prepare_direct_workers(
            &config,
            PoolConfig {
                test_echo: true,
                ..Default::default()
            },
        )
        .await
        .expect("prepare workers");
        let host = DirectHost::new(
            config,
            layout,
            None,
            Some(tempdir.path().join("cache")),
            &workers,
        )
        .await
        .expect("create direct host");

        let job_id = host
            .submit_submission(transcribe_submission(&source_path, &output_path))
            .await
            .expect("submit submission");
        let submitted = host.job_info(&job_id).await.expect("submitted job info");
        assert_eq!(submitted.status, JobStatus::Queued);
        assert_eq!(submitted.total_files, 1);

        host.run_job(&job_id).await.expect("run direct job");

        let final_info = host.job_info(&job_id).await.expect("final job info");
        assert_eq!(final_info.status, JobStatus::Completed);
        assert_eq!(final_info.completed_files, 1);

        let detail = host.job_detail(&job_id).await.expect("job detail");
        assert!(detail.paths_mode);
        assert_eq!(detail.results.len(), 1);
    }

    #[tokio::test]
    async fn direct_host_persists_debug_artifact_summary() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        std::fs::create_dir_all(layout.state_dir()).expect("create state dir");

        let input_dir = tempdir.path().join("input");
        let output_dir = tempdir.path().join("output");
        std::fs::create_dir_all(&input_dir).expect("create input dir");
        std::fs::create_dir_all(&output_dir).expect("create output dir");

        let source_path = input_dir.join("sample.wav");
        let output_path = output_dir.join("sample.cha");
        tokio::fs::write(&source_path, b"RIFF")
            .await
            .expect("write input");

        let config = ServerConfig::default();
        let workers = prepare_direct_workers(
            &config,
            PoolConfig {
                test_echo: true,
                ..Default::default()
            },
        )
        .await
        .expect("prepare workers");
        let host = DirectHost::new(
            config,
            layout,
            None,
            Some(tempdir.path().join("cache")),
            &workers,
        )
        .await
        .expect("create direct host");

        let job_id = host
            .submit_submission(transcribe_submission(&source_path, &output_path))
            .await
            .expect("submit submission");

        let artifacts = host
            .job_debug_artifacts(&job_id)
            .await
            .expect("job debug artifacts");
        assert_eq!(artifacts.job_id, job_id);
        assert!(artifacts.staging_dir.ends_with(job_id.to_string()));
        assert!(artifacts.staging_dir.join("debug-artifacts.json").exists());
        assert!(artifacts.trace_file.is_none());
    }

    #[tokio::test]
    async fn direct_host_exports_debug_traces_into_staging_dir() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let layout = RuntimeLayout::from_state_dir(tempdir.path().join("state"));
        std::fs::create_dir_all(layout.state_dir()).expect("create state dir");

        let input_dir = tempdir.path().join("input");
        let output_dir = tempdir.path().join("output");
        std::fs::create_dir_all(&input_dir).expect("create input dir");
        std::fs::create_dir_all(&output_dir).expect("create output dir");

        let source_path = input_dir.join("sample.wav");
        let output_path = output_dir.join("sample.cha");
        tokio::fs::write(&source_path, b"RIFF")
            .await
            .expect("write input");

        let config = ServerConfig::default();
        let workers = prepare_direct_workers(
            &config,
            PoolConfig {
                test_echo: true,
                ..Default::default()
            },
        )
        .await
        .expect("prepare workers");
        let host = DirectHost::new(
            config,
            layout,
            None,
            Some(tempdir.path().join("cache")),
            &workers,
        )
        .await
        .expect("create direct host");

        let job_id = host
            .submit_submission(transcribe_submission(&source_path, &output_path))
            .await
            .expect("submit submission");
        host.store
            .trace_store()
            .insert(job_id.clone(), JobTraces::default())
            .await;

        let artifacts = host
            .job_debug_artifacts(&job_id)
            .await
            .expect("job debug artifacts");
        let trace_file = artifacts.trace_file.expect("trace file path");
        assert!(trace_file.ends_with("debug-traces.json"));
        assert!(trace_file.exists());
    }
}
