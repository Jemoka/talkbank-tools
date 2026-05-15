use std::sync::Arc;

use tracing::warn;

use crate::planning;
use crate::runner::DispatchHostContext;
use crate::runner::util::{FileRunTracker, FileStage};
use crate::scheduling::WorkUnitKind;
use crate::store::{RunnerJobSnapshot, unix_now};

use super::text_io::{load_text_inputs, write_text_results};
use super::worker_gateway::WorkerGateway;

/// Default cap on concurrent per-file utseg tasks when the server config
/// does not set `max_workers_per_job`. Matches the FA pipeline default.
const DEFAULT_UTSEG_FILE_PARALLELISM: usize = 4;

/// Per-file utseg dispatch with bounded parallelism.
///
/// Replaces the prior "pool everything, batch through worker, write at
/// end" pattern that was structurally fragile to any interruption — a
/// daemon redeploy mid-batch could vaporize the entire run's work.
/// Each file gets its own `gateway.utseg_batch` call with a single-file
/// vec and writes its result to disk before joining; an interruption at
/// any point loses at most the files currently in flight rather than
/// the whole batch.
///
/// Bounded by `plan.kernel_plan.file_parallelism_hint` (clamped to ≥1)
/// — same heuristic as `fa_pipeline.rs`. On macOS without MPS the
/// underlying BERT inference is single-thread CPU-bound so the cap
/// effectively limits how many files share that single thread, but
/// when worker pools grow or GPU comes back the same code scales
/// without architectural change. The gateway is passed in as
/// `Arc<dyn WorkerGateway>` so spawned tasks can each hold a clone.
pub(crate) async fn dispatch_utseg_job(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    gateway: Arc<dyn WorkerGateway>,
    should_merge_abbrev: bool,
    allow_stanza_fallback: bool,
) -> Result<(), crate::error::ServerError> {
    let plan = planning::build_job_plan(job).map_err(|error| {
        crate::error::ServerError::Validation(format!("Utseg planning failed: {error}"))
    })?;
    let plan = Arc::new(plan);
    let sink = host.sink().clone();
    let started_at = unix_now();

    let inputs = load_text_inputs(job, host, false).await;
    if inputs.file_texts.is_empty() {
        return Ok(());
    }

    for file in &job.pending_files {
        FileRunTracker::new(sink.as_ref(), &job.identity.job_id, file.filename.as_ref())
            .begin_first_attempt(WorkUnitKind::BatchInfer, started_at, FileStage::Segmenting)
            .await;
    }

    // No silent eng fallback. Utseg requires a concrete ISO language —
    // submission validation accepts `Resolved(_)`; if anything else slipped
    // through, surface a typed error rather than tag every output with
    // English.
    let lang = job.dispatch.lang.as_resolved().cloned().ok_or_else(|| {
        crate::error::ServerError::Validation(format!(
            "utseg requires `--lang <iso3>`; got '{}'. Re-submit with an \
             explicit language (e.g. `--lang eng`).",
            job.dispatch.lang,
        ))
    })?;

    // Bounded-parallelism per-file dispatch. Same shape as
    // `fa_pipeline.rs`: Semaphore caps the number of concurrent file
    // tasks; each task takes its permit, calls the gateway with a
    // single-file vec, writes the result, and releases the permit on
    // drop. JoinSet collects task completions; we await each to
    // surface panics if any.
    //
    // Parallelism limit derives from the server config's
    // `max_workers_per_job` (defaulting to a small cap when unset),
    // matching the convention used by FA. Single-thread BERT
    // inference on macOS still bottlenecks at the worker process,
    // but with multiple workers in the pool the cap effectively
    // becomes the file-fan-out and gives real wall-clock benefit.
    let file_parallelism = host
        .config()
        .max_workers_per_job
        .map(|n| n.max(1) as usize)
        .unwrap_or(DEFAULT_UTSEG_FILE_PARALLELISM);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(file_parallelism));
    let mut joinset: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

    for file_input in inputs.file_texts {
        if job.cancel_token.is_cancelled() {
            break;
        }
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                // Semaphore closed during shutdown — abandon further
                // submissions but await tasks already spawned.
                break;
            }
        };
        let gateway_for_task = Arc::clone(&gateway);
        let lang = lang.clone();
        let host_for_task = host.clone();
        let job_for_task = job.clone();
        let plan_for_task = Arc::clone(&plan);
        let merge_abbrev = should_merge_abbrev;
        joinset.spawn(async move {
            let _permit = permit; // released on drop after the task completes
            let single = vec![file_input];
            let results = gateway_for_task
                .utseg_batch(&single, &lang, allow_stanza_fallback)
                .await;
            write_text_results(
                &job_for_task,
                &host_for_task,
                &plan_for_task,
                results,
                merge_abbrev,
                "Utseg",
            )
            .await;
        });
    }

    while let Some(join_result) = joinset.join_next().await {
        if let Err(error) = join_result {
            // A spawned file task panicked. write_text_results would
            // normally surface per-file failures via the sink; a
            // JoinError here means the task itself terminated
            // abnormally before reaching the sink. Log so it's not
            // silent.
            warn!(
                job_id = %job.identity.job_id,
                error = %error,
                "Utseg per-file task panicked"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::Mutex;

    use crate::chat_ops::morphosyntax_ops::MwtDict;
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::api::{
        CorrelationId, DisplayPath, JobId, LanguageCode3, LanguageSpec, NumSpeakers,
        ReleasedCommand, WorkerLanguage,
    };
    use crate::capability::WorkerCapabilitySnapshot;
    use crate::execution::worker_gateway::MorphotagRuntimeOptions;
    use crate::options::{CommandOptions, CommonOptions, UtsegOptions};
    use crate::runner::DispatchHostContext;
    use crate::store::PendingJobFile;
    use crate::text_batch::{TextBatchFileInput, TextBatchFileResult, TextBatchFileResults};

    #[derive(Default)]
    struct FakeUtsegGateway {
        state: Mutex<FakeUtsegState>,
    }

    #[derive(Default)]
    struct FakeUtsegState {
        batch_calls: usize,
        batch_sizes: Vec<usize>,
    }

    #[async_trait]
    impl WorkerGateway for FakeUtsegGateway {
        async fn ensure_command_capabilities(
            &self,
            _command: ReleasedCommand,
            _lang: WorkerLanguage,
            _engine_overrides: &str,
        ) -> Result<WorkerCapabilitySnapshot, String> {
            unreachable!()
        }

        async fn morphotag_for_compare(
            &self,
            _chat_text: &str,
            _lang: &LanguageCode3,
            _mwt: &MwtDict,
        ) -> Result<String, crate::error::ServerError> {
            unreachable!()
        }

        async fn morphotag_single(
            &self,
            _chat_text: &str,
            _before_text: Option<&str>,
            _lang: &LanguageCode3,
            _options: MorphotagRuntimeOptions,
        ) -> Result<String, crate::error::ServerError> {
            unreachable!()
        }

        async fn utseg_batch(
            &self,
            files: &[TextBatchFileInput],
            _lang: &LanguageCode3,
            _allow_stanza_fallback: bool,
        ) -> TextBatchFileResults {
            let mut state = self.state.lock().unwrap();
            state.batch_calls += 1;
            state.batch_sizes.push(files.len());
            files
                .iter()
                .map(|file| TextBatchFileResult::ok(file.filename.clone(), file.chat_text.clone()))
                .collect()
        }

        async fn translate_batch(
            &self,
            _files: &[TextBatchFileInput],
            _lang: &LanguageCode3,
        ) -> TextBatchFileResults {
            unreachable!()
        }

        async fn coref_batch(
            &self,
            _files: &[TextBatchFileInput],
            _lang: &LanguageCode3,
        ) -> TextBatchFileResults {
            unreachable!()
        }
    }

    fn utseg_snapshot(staging_dir: &std::path::Path, merge_abbrev: bool) -> RunnerJobSnapshot {
        let text = "@UTF8\n@Begin\n*PAR:\tF B I .\n@End\n";
        let input_dir = staging_dir.join("input");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::write(input_dir.join("a.cha"), text).unwrap();
        std::fs::write(input_dir.join("b.cha"), text).unwrap();
        RunnerJobSnapshot {
            identity: crate::store::RunnerJobIdentity {
                job_id: JobId::from("job-utseg"),
                correlation_id: CorrelationId::from("corr-utseg"),
            },
            dispatch: crate::store::RunnerDispatchConfig {
                command: ReleasedCommand::Utseg,
                lang: LanguageSpec::Resolved(LanguageCode3::eng()),
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Utseg(UtsegOptions {
                    common: CommonOptions::default(),
                    merge_abbrev: merge_abbrev.into(),
                    utseg_fallback: false.into(),
                }),
                runtime_state: BTreeMap::new(),
                debug_traces: false,
            },
            filesystem: crate::store::RunnerFilesystemConfig {
                paths_mode: false,
                source_paths: Vec::new(),
                output_paths: Vec::new(),
                before_paths: Vec::new(),
                staging_dir: batchalign_types::paths::ServerPath::new(
                    staging_dir.display().to_string(),
                ),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: batchalign_types::paths::ClientPath::new(
                    staging_dir.display().to_string(),
                ),
            },
            cancel_token: CancellationToken::new(),
            pending_files: vec![
                PendingJobFile {
                    file_index: 0,
                    filename: DisplayPath::from("a.cha"),
                    has_chat: true,
                },
                PendingJobFile {
                    file_index: 1,
                    filename: DisplayPath::from("b.cha"),
                    has_chat: true,
                },
            ],
        }
    }

    fn host() -> DispatchHostContext {
        let (tx, _rx) = tokio::sync::broadcast::channel(crate::ws::BROADCAST_CAPACITY);
        DispatchHostContext::from_store(Arc::new(crate::store::JobStore::new(
            crate::config::ServerConfig::default(),
            None,
            tx,
        )))
    }

    /// Per-file dispatch: each file produces its own gateway call, never
    /// pooled across files. Two files → two gateway calls of size 1 each.
    /// This is the property that gives utseg incremental writeback and
    /// failure isolation.
    #[tokio::test]
    async fn utseg_dispatches_one_gateway_call_per_file() {
        let temp = tempfile::tempdir().unwrap();
        let host = host();
        let gateway = Arc::new(FakeUtsegGateway::default());
        let job = utseg_snapshot(temp.path(), false);

        dispatch_utseg_job(
            &job,
            &host,
            Arc::clone(&gateway) as Arc<dyn WorkerGateway>,
            false,
            false,
        )
        .await
        .expect("utseg dispatch");

        let state = gateway.state.lock().unwrap();
        assert_eq!(state.batch_calls, 2, "one gateway call per file");
        // Each call carries exactly one file. With parallelism, the
        // per-call payload is single-file regardless of order; sort the
        // observed sizes for a stable assertion.
        let mut sizes = state.batch_sizes.clone();
        sizes.sort();
        assert_eq!(sizes, vec![1, 1], "each call carries one file");
    }

    #[tokio::test]
    async fn utseg_write_path_can_merge_abbrev() {
        let temp = tempfile::tempdir().unwrap();
        let host = host();
        let gateway = Arc::new(FakeUtsegGateway::default());
        let job = utseg_snapshot(temp.path(), true);

        dispatch_utseg_job(&job, &host, gateway as Arc<dyn WorkerGateway>, true, false)
            .await
            .expect("utseg dispatch");

        let output = std::fs::read_to_string(temp.path().join("output").join("a.cha")).unwrap();
        assert!(output.contains("*PAR:\tFBI ."));
    }

    // Note: the prior `utseg_writes_files_incrementally` probing-
    // gateway test relied on sequential per-file dispatch (the second
    // gateway call could observe the first's writeback). Under
    // bounded parallelism (default 4) the calls race and the strict
    // before/after check is no longer well-defined. The
    // incremental-writeback property is now verified empirically on
    // real corpus runs (see operational workspace's
    // `utseg-most-fullrun-attempt` follow-up: 9 output files were on
    // disk while the worker was still processing 876 more — a
    // property impossible under the prior batched dispatch).
}
