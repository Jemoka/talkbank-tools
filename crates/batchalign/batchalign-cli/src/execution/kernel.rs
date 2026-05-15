use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::chat_ops::morphosyntax_ops::MwtDict;
use async_trait::async_trait;
use tracing::warn;

use crate::api::{DisplayPath, ReleasedCommand};
use crate::compare::{
    CompareMaterializedOutputs, is_gold_file, process_compare_morphotagged_main,
    template_gold_path_for,
};
use crate::planning::{self, JobPlan};
use crate::recipe_runner::materialize::MaterializedArtifactRole;
use crate::recipe_runner::recipe::RecipeStageId;
use crate::recipe_runner::runtime::{
    ChatOutputTarget, output_write_path, write_text_output_artifact,
};
use crate::recipe_runner::work_unit::{CompareWorkUnit, PlannedWorkUnit};
use crate::runner::DispatchHostContext;
use crate::runner::util::{FileRunTracker, FileStage, classify_server_error};
use crate::scheduling::{FailureCategory, WorkUnitKind};
use crate::store::{RunnerJobSnapshot, unix_now};

use super::worker_gateway::WorkerGateway;

/// Immutable execution inputs shared across stages for one job.
struct ExecutionContext<'a> {
    pub(crate) job: &'a RunnerJobSnapshot,
    pub(crate) host: &'a DispatchHostContext,
    pub(crate) gateway: &'a dyn WorkerGateway,
    pub(crate) mwt: &'a MwtDict,
    pub(crate) should_merge_abbrev: bool,
}

/// Stage executor interface used by the new execution kernel.
#[async_trait]
trait StageExecutor {
    /// Run one stage for the current work unit.
    async fn run_stage(
        &self,
        stage: RecipeStageId,
        state: &mut CompareExecutionState,
        plan: &JobPlan,
        ctx: &ExecutionContext<'_>,
    ) -> Result<(), crate::error::ServerError>;
}

/// Minimal execution kernel for recipe-owned commands.
struct ExecutionKernel {
    stage_executor: Box<dyn StageExecutor + Send + Sync>,
}

impl ExecutionKernel {
    /// Build a kernel with one stage executor implementation.
    pub(crate) fn new(stage_executor: Box<dyn StageExecutor + Send + Sync>) -> Self {
        Self { stage_executor }
    }

    /// Run one immutable job plan through the stage executor.
    pub(crate) async fn run(
        &self,
        plan: &JobPlan,
        ctx: &ExecutionContext<'_>,
    ) -> Result<(), crate::error::ServerError> {
        match plan.spec.command {
            ReleasedCommand::Compare => self.run_compare(plan, ctx).await,
            command => Err(crate::error::ServerError::Validation(format!(
                "execution kernel does not yet support command '{command}'"
            ))),
        }
    }

    async fn run_compare(
        &self,
        plan: &JobPlan,
        ctx: &ExecutionContext<'_>,
    ) -> Result<(), crate::error::ServerError> {
        let file_index_by_display: HashMap<DisplayPath, usize> = ctx
            .job
            .pending_files
            .iter()
            .map(|file| (file.filename.clone(), file.file_index))
            .collect();
        let sink = ctx.host.sink().clone();
        let started_at = unix_now();
        let mut consolidated_rows = Vec::new();

        for file in &ctx.job.pending_files {
            let lifecycle = FileRunTracker::new(
                sink.as_ref(),
                &ctx.job.identity.job_id,
                file.filename.as_ref(),
            );
            lifecycle
                .begin_first_attempt(WorkUnitKind::BatchInfer, started_at, FileStage::Comparing)
                .await;
            if is_gold_file(file.filename.as_ref()) {
                lifecycle.complete_without_result(started_at).await;
            }
        }

        for work_unit in &plan.work_units {
            let PlannedWorkUnit::Compare(unit) = work_unit else {
                continue;
            };
            let Some(file_index) = file_index_by_display.get(&unit.main.display_path).copied()
            else {
                continue;
            };
            let mut state = CompareExecutionState::new(unit.clone(), file_index);
            let stage_result = async {
                for stage in plan.spec.recipe.stages {
                    self.stage_executor
                        .run_stage(stage.id, &mut state, plan, ctx)
                        .await?;
                }
                Ok::<(), crate::error::ServerError>(())
            }
            .await;
            if let Err(error) = stage_result {
                state
                    .lifecycle(ctx)
                    .fail(
                        &error.to_string(),
                        classify_server_error(&error),
                        unix_now(),
                    )
                    .await;
            } else if let Some(row) = state.consolidated_metrics.take() {
                consolidated_rows.push(row);
            }
        }

        if !consolidated_rows.is_empty() {
            write_consolidated_compare_csv(&ctx.job.filesystem, &consolidated_rows).await?;
        }

        Ok(())
    }
}

/// Runner-owned dispatch entrypoint for the first migrated command family.
pub(crate) async fn dispatch_compare_job(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    gateway: &dyn WorkerGateway,
    mwt: &MwtDict,
    should_merge_abbrev: bool,
) -> Result<(), crate::error::ServerError> {
    let plan = match planning::build_job_plan(job) {
        Ok(plan) => plan,
        Err(error) => {
            let sink = host.sink().clone();
            let failed_at = unix_now();
            for file in &job.pending_files {
                let lifecycle = FileRunTracker::new(
                    sink.as_ref(),
                    &job.identity.job_id,
                    file.filename.as_ref(),
                );
                if is_gold_file(file.filename.as_ref()) {
                    lifecycle.complete_without_result(failed_at).await;
                    continue;
                }
                lifecycle
                    .fail(
                        &format!("Compare planning failed: {error}"),
                        FailureCategory::Validation,
                        failed_at,
                    )
                    .await;
            }
            return Ok(());
        }
    };

    let ctx = ExecutionContext {
        job,
        host,
        gateway,
        mwt,
        should_merge_abbrev,
    };
    ExecutionKernel::new(Box::new(CompareStageExecutor))
        .run(&plan, &ctx)
        .await
}

struct CompareExecutionState {
    unit: CompareWorkUnit,
    file_index: usize,
    main_text: Option<String>,
    gold_text: Option<String>,
    morphotagged_main: Option<String>,
    outputs: Option<CompareMaterializedOutputs>,
    consolidated_metrics: Option<ConsolidatedCompareMetricsRow>,
}

impl CompareExecutionState {
    fn new(unit: CompareWorkUnit, file_index: usize) -> Self {
        Self {
            unit,
            file_index,
            main_text: None,
            gold_text: None,
            morphotagged_main: None,
            outputs: None,
            consolidated_metrics: None,
        }
    }

    fn lifecycle<'a>(&'a self, ctx: &'a ExecutionContext<'_>) -> FileRunTracker<'a> {
        FileRunTracker::new(
            ctx.host.sink().as_ref(),
            &ctx.job.identity.job_id,
            self.unit.main.display_path.as_ref(),
        )
    }
}

struct CompareStageExecutor;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConsolidatedCompareMetricsRow {
    file: String,
    metrics: Vec<(String, String)>,
}

#[async_trait]
impl StageExecutor for CompareStageExecutor {
    async fn run_stage(
        &self,
        stage: RecipeStageId,
        state: &mut CompareExecutionState,
        plan: &JobPlan,
        ctx: &ExecutionContext<'_>,
    ) -> Result<(), crate::error::ServerError> {
        match stage {
            RecipeStageId::PlanWorkUnits => Ok(()),
            RecipeStageId::ReadChatInputs => {
                state.lifecycle(ctx).stage(FileStage::Reading).await;
                state.main_text = Some(
                    tokio::fs::read_to_string(&state.unit.main.source_path)
                        .await
                        .map_err(|error| {
                            crate::error::ServerError::Validation(format!(
                                "failed to read compare input {}: {error}",
                                state.unit.main.display_path
                            ))
                        })?,
                );
                Ok(())
            }
            RecipeStageId::ReadReferenceInputs => {
                state.lifecycle(ctx).stage(FileStage::Reading).await;
                let template_gold_source = state
                    .unit
                    .main
                    .source_path
                    .with_file_name("template.gold.cha");
                state.gold_text = Some(match tokio::fs::read_to_string(&state.unit.gold.source_path).await {
                    Ok(text) => text,
                    Err(_) => tokio::fs::read_to_string(&template_gold_source)
                        .await
                        .map_err(|_| {
                            crate::error::ServerError::Validation(format!(
                                "No gold .cha file found for comparison. main: {}, expected: {} or {}",
                                state.unit.main.display_path,
                                state.unit.gold.display_path,
                                template_gold_path_for(state.unit.main.display_path.as_ref()),
                            ))
                        })?,
                });
                Ok(())
            }
            RecipeStageId::Morphosyntax => {
                state
                    .lifecycle(ctx)
                    .stage(FileStage::AnalyzingMorphosyntax)
                    .await;
                let main_text = state.main_text.as_deref().ok_or_else(|| {
                    crate::error::ServerError::Validation(
                        "compare morphosyntax stage ran before input read".into(),
                    )
                })?;
                // Same job-level fallback as `infer_batched.rs`: real
                // per-file resolution lives inside `collect_payloads`;
                // this is just the worker-pool / label key. Logged when
                // the sentinel fires so misrouting is auditable.
                let fallback_lang = crate::api::LanguageCode3::eng();
                let lang = ctx.job.dispatch.lang.as_resolved().unwrap_or_else(|| {
                    tracing::warn!(
                        job_id = %ctx.job.identity.job_id,
                        "compare morphotag stage: no resolved job-level \
                         language; using `eng` as worker-pool fallback. \
                         Per-file resolution derives from file headers.",
                    );
                    &fallback_lang
                });
                state.morphotagged_main = Some(
                    ctx.gateway
                        .morphotag_for_compare(main_text, lang, ctx.mwt)
                        .await?,
                );
                Ok(())
            }
            RecipeStageId::CompareAlign => {
                state.lifecycle(ctx).stage(FileStage::Comparing).await;
                let morphotagged_main = state.morphotagged_main.as_deref().ok_or_else(|| {
                    crate::error::ServerError::Validation(
                        "compare alignment stage ran before morphosyntax".into(),
                    )
                })?;
                let gold_text = state.gold_text.as_deref().ok_or_else(|| {
                    crate::error::ServerError::Validation(
                        "compare alignment stage ran before reference read".into(),
                    )
                })?;
                state.outputs = Some(process_compare_morphotagged_main(
                    morphotagged_main,
                    gold_text,
                )?);
                Ok(())
            }
            RecipeStageId::CompareMetrics => Ok(()),
            RecipeStageId::SerializeChat => {
                state.lifecycle(ctx).stage(FileStage::Finalizing).await;
                Ok(())
            }
            RecipeStageId::MaterializeOutputs => {
                state.lifecycle(ctx).stage(FileStage::Writing).await;
                let outputs = state.outputs.take().ok_or_else(|| {
                    crate::error::ServerError::Validation(
                        "compare output materialization ran before compare outputs existed".into(),
                    )
                })?;
                let Some(artifacts) =
                    planning::artifact_set_for_source(plan, &state.unit.main.display_path)
                else {
                    return Err(crate::error::ServerError::Validation(format!(
                        "compare job plan was missing artifacts for {}",
                        state.unit.main.display_path
                    )));
                };
                let finished_at = unix_now();
                for artifact in &artifacts.files {
                    match artifact.role {
                        MaterializedArtifactRole::Primary => {
                            let chat_output = if ctx.should_merge_abbrev {
                                apply_merge_abbrev_local(&outputs.chat_output)
                            } else {
                                outputs.chat_output.clone()
                            };
                            let target = ChatOutputTarget::new(
                                &ctx.job.filesystem,
                                state.file_index,
                                &artifact.display_path,
                            );
                            if let Err(error) =
                                write_text_output_artifact(&target, &chat_output).await
                            {
                                warn!(
                                    error = %error,
                                    "Failed to write compare output"
                                );
                            }
                            state
                                .lifecycle(ctx)
                                .complete_with_result(
                                    artifact.display_path.clone(),
                                    artifact.content_type,
                                    finished_at,
                                )
                                .await;
                        }
                        MaterializedArtifactRole::Sidecar => {
                            let csv_path = output_write_path(
                                &ctx.job.filesystem,
                                state.file_index,
                                &artifact.display_path,
                            );
                            if let Err(error) =
                                tokio::fs::write(&csv_path, &outputs.metrics_csv).await
                            {
                                warn!(error = %error, "Failed to write compare CSV");
                            }
                            state.consolidated_metrics = Some(parse_consolidated_metrics_row(
                                &artifact.display_path,
                                &outputs.metrics_csv,
                            )?);
                        }
                    }
                }
                Ok(())
            }
            other => Err(crate::error::ServerError::Validation(format!(
                "compare kernel does not yet support stage '{other}'"
            ))),
        }
    }
}

fn apply_merge_abbrev_local(chat_text: &str) -> String {
    let parser = crate::chat_parser();
    let (mut file, _) = talkbank_transform::parse::parse_lenient(&parser, chat_text);
    talkbank_transform::merge_abbreviations(&mut file);
    talkbank_transform::serialize::to_chat_string(&file)
}

fn parse_consolidated_metrics_row(
    artifact_display_path: &DisplayPath,
    metrics_csv: &str,
) -> Result<ConsolidatedCompareMetricsRow, crate::error::ServerError> {
    let file = Path::new(artifact_display_path.as_ref())
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            crate::error::ServerError::Validation(format!(
                "compare metrics artifact had invalid display path {}",
                artifact_display_path
            ))
        })?
        .replace(".compare.csv", ".cha");
    let mut metrics = Vec::new();
    for line in metrics_csv.lines().skip(1) {
        let Some((key, value)) = line.split_once(',') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = normalize_compare_csv_value(&key, value.trim());
        if !key.is_empty() {
            metrics.push((key, value));
        }
    }
    Ok(ConsolidatedCompareMetricsRow { file, metrics })
}

fn normalize_compare_csv_value(key: &str, value: &str) -> String {
    if key != "wer" && key != "accuracy" {
        return value.to_string();
    }

    let Ok(parsed) = value.parse::<f64>() else {
        return value.to_string();
    };
    let mut normalized = format!("{parsed:.4}");
    while normalized.contains('.') && normalized.ends_with('0') {
        normalized.pop();
    }
    if normalized.ends_with('.') {
        normalized.push('0');
    }
    normalized
}

async fn write_compare_text_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, content).await
}

fn compare_output_root(filesystem: &crate::store::RunnerFilesystemConfig) -> PathBuf {
    if filesystem.paths_mode && !filesystem.output_paths.is_empty() {
        let first_output = filesystem.output_paths[0].assume_shared_filesystem();
        return Path::new(first_output.as_str())
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
    }

    filesystem.staging_dir.join("output").as_path().to_owned()
}

async fn write_consolidated_compare_csv(
    filesystem: &crate::store::RunnerFilesystemConfig,
    rows: &[ConsolidatedCompareMetricsRow],
) -> Result<(), crate::error::ServerError> {
    let mut header_keys = Vec::new();
    for row in rows {
        for (key, _) in &row.metrics {
            if !header_keys.contains(key) {
                header_keys.push(key.clone());
            }
        }
    }

    let mut output = String::from("file");
    for key in &header_keys {
        output.push(',');
        output.push_str(key);
    }
    output.push('\n');

    for row in rows {
        let values: BTreeMap<_, _> = row.metrics.iter().cloned().collect();
        output.push_str(&row.file);
        for key in &header_keys {
            output.push(',');
            if let Some(value) = values.get(key) {
                output.push_str(value);
            }
        }
        output.push('\n');
    }

    let primary_path = compare_output_root(filesystem).join("compare.csv");
    write_compare_text_file(&primary_path, &output)
        .await
        .map_err(|error| {
            crate::error::ServerError::Persistence(format!(
                "failed to write consolidated compare.csv: {error}"
            ))
        })?;

    let staged_path = filesystem.staging_dir.join("output").join("compare.csv");
    let staged_path = staged_path.as_path().to_owned();
    if staged_path != primary_path {
        write_compare_text_file(&staged_path, &output)
            .await
            .map_err(|error| {
                crate::error::ServerError::Persistence(format!(
                    "failed to write staged consolidated compare.csv: {error}"
                ))
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::api::{CorrelationId, JobId, LanguageSpec, NumSpeakers};
    use crate::options::{CommandOptions, CommonOptions, CompareOptions};
    use crate::planning::build_job_plan;
    use crate::store::{
        PendingJobFile, RunnerDispatchConfig, RunnerFilesystemConfig, RunnerJobIdentity,
    };

    #[derive(Default)]
    struct FakeGateway;

    #[async_trait]
    impl WorkerGateway for FakeGateway {
        async fn ensure_command_capabilities(
            &self,
            _command: ReleasedCommand,
            _lang: crate::api::WorkerLanguage,
            _engine_overrides: &str,
        ) -> Result<crate::capability::WorkerCapabilitySnapshot, String> {
            unreachable!("test does not call ensure_command_capabilities")
        }

        async fn morphotag_for_compare(
            &self,
            chat_text: &str,
            _lang: &crate::api::LanguageCode3,
            _mwt: &MwtDict,
        ) -> Result<String, crate::error::ServerError> {
            Ok(chat_text.to_string())
        }

        async fn morphotag_single(
            &self,
            _chat_text: &str,
            _before_text: Option<&str>,
            _lang: &crate::api::LanguageCode3,
            _options: crate::execution::MorphotagRuntimeOptions,
        ) -> Result<String, crate::error::ServerError> {
            unreachable!("compare tests do not call morphotag_single")
        }

        async fn utseg_batch(
            &self,
            _files: &[crate::text_batch::TextBatchFileInput],
            _lang: &crate::api::LanguageCode3,
            _allow_stanza_fallback: bool,
        ) -> crate::text_batch::TextBatchFileResults {
            unreachable!("compare tests do not call utseg_batch")
        }

        async fn translate_batch(
            &self,
            _files: &[crate::text_batch::TextBatchFileInput],
            _lang: &crate::api::LanguageCode3,
        ) -> crate::text_batch::TextBatchFileResults {
            unreachable!("compare tests do not call translate_batch")
        }

        async fn coref_batch(
            &self,
            _files: &[crate::text_batch::TextBatchFileInput],
            _lang: &crate::api::LanguageCode3,
        ) -> crate::text_batch::TextBatchFileResults {
            unreachable!("compare tests do not call coref_batch")
        }
    }

    fn compare_snapshot(staging_dir: &std::path::Path) -> RunnerJobSnapshot {
        RunnerJobSnapshot {
            identity: RunnerJobIdentity {
                job_id: JobId::from("job-compare-kernel"),
                correlation_id: CorrelationId::from("corr-compare-kernel"),
            },
            dispatch: RunnerDispatchConfig {
                command: ReleasedCommand::Compare,
                lang: LanguageSpec::Resolved(crate::api::LanguageCode3::eng()),
                num_speakers: NumSpeakers(1),
                options: CommandOptions::Compare(CompareOptions {
                    common: CommonOptions::default(),
                    merge_abbrev: false.into(),
                }),
                runtime_state: BTreeMap::new(),
                debug_traces: false,
            },
            filesystem: RunnerFilesystemConfig {
                paths_mode: false,
                source_paths: Vec::new(),
                output_paths: Vec::new(),
                before_paths: Vec::new(),
                staging_dir: batchalign_types::paths::ServerPath::new(staging_dir),
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: batchalign_types::paths::ClientPath::new("/source"),
            },
            cancel_token: CancellationToken::new(),
            pending_files: vec![PendingJobFile {
                file_index: 0,
                filename: DisplayPath::from("sample.cha"),
                has_chat: true,
            }],
        }
    }

    #[tokio::test]
    async fn compare_kernel_writes_primary_and_sidecar_outputs() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let input_dir = tempdir.path().join("input");
        tokio::fs::create_dir_all(&input_dir)
            .await
            .expect("input dir");
        tokio::fs::write(
            input_dir.join("sample.cha"),
            "@UTF8\n@Begin\n*PAR:\thello there .\n@End\n",
        )
        .await
        .expect("main");
        tokio::fs::write(
            input_dir.join("sample.gold.cha"),
            "@UTF8\n@Begin\n*PAR:\thello there .\n@End\n",
        )
        .await
        .expect("gold");

        let snapshot = compare_snapshot(tempdir.path());
        let plan = build_job_plan(&snapshot).expect("plan");
        let (_tx, _rx) = tokio::sync::broadcast::channel(crate::ws::BROADCAST_CAPACITY);
        let store = std::sync::Arc::new(crate::store::JobStore::new(
            crate::config::ServerConfig::default(),
            None,
            _tx,
        ));
        let host = DispatchHostContext::from_store(store);
        let ctx = ExecutionContext {
            job: &snapshot,
            host: &host,
            gateway: &FakeGateway,
            mwt: &MwtDict::default(),
            should_merge_abbrev: false,
        };

        ExecutionKernel::new(Box::new(CompareStageExecutor))
            .run(&plan, &ctx)
            .await
            .expect("run compare kernel");

        let primary = tempdir.path().join("output").join("sample.cha");
        let csv = tempdir.path().join("output").join("sample.compare.csv");
        let consolidated = tempdir.path().join("output").join("compare.csv");
        assert!(primary.exists());
        assert!(csv.exists());
        assert!(consolidated.exists());
    }
}
