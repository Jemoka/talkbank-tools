use std::collections::HashMap;
use std::path::PathBuf;

use crate::planning;
use crate::recipe_runner::materialize::MaterializedArtifactRole;
use crate::runner::DispatchHostContext;
use crate::runner::util::{FileRunTracker, FileStage, set_file_progress};
use crate::scheduling::FailureCategory;
use crate::store::{PendingJobFile, RunnerJobSnapshot, unix_now};
use crate::text_batch::{TextBatchFileInput, TextBatchFileResults};

pub(crate) struct LoadedTextInputs {
    pub(crate) file_texts: Vec<TextBatchFileInput>,
    pub(crate) before_texts: HashMap<String, String>,
}

pub(crate) async fn load_text_inputs(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    load_before_texts: bool,
) -> LoadedTextInputs {
    let mut loaded = LoadedTextInputs {
        file_texts: Vec::with_capacity(job.pending_files.len()),
        before_texts: HashMap::new(),
    };

    for file in &job.pending_files {
        let lifecycle = FileRunTracker::new(
            host.sink().as_ref(),
            &job.identity.job_id,
            file.filename.as_ref(),
        );
        lifecycle.stage(FileStage::Reading).await;

        let read_path = resolve_input_path(job, file);
        match tokio::fs::read_to_string(&read_path).await {
            Ok(text) => {
                if load_before_texts
                    && let Some(before_path) = resolve_before_path(job, file)
                    && let Ok(before_text) = tokio::fs::read_to_string(before_path).await
                {
                    loaded
                        .before_texts
                        .insert(file.filename.to_string(), before_text);
                }
                loaded
                    .file_texts
                    .push(TextBatchFileInput::new(file.filename.clone(), text));
            }
            Err(error) => {
                lifecycle
                    .fail(
                        &format!("Failed to read input: {error}"),
                        FailureCategory::InputMissing,
                        unix_now(),
                    )
                    .await;
            }
        }
    }

    loaded
}

pub(crate) async fn write_text_results(
    job: &RunnerJobSnapshot,
    host: &DispatchHostContext,
    plan: &planning::JobPlan,
    results: TextBatchFileResults,
    should_merge_abbrev: bool,
    missing_artifact_label: &str,
) {
    let sink = host.sink().clone();
    let total_results = results.len() as i64;
    for (result_idx, file_result) in results.into_iter().enumerate() {
        let lifecycle = FileRunTracker::new(
            sink.as_ref(),
            &job.identity.job_id,
            file_result.filename.as_ref(),
        );
        let file_index = job
            .pending_files
            .iter()
            .find(|file| file.filename == file_result.filename)
            .map(|file| file.file_index)
            .unwrap_or(0);
        match file_result.result {
            Ok(output_chat) => {
                set_file_progress(
                    sink.as_ref(),
                    &job.identity.job_id,
                    file_result.filename.as_ref(),
                    FileStage::Writing,
                    Some(result_idx as i64 + 1),
                    Some(total_results),
                )
                .await;
                let Some(artifacts) =
                    planning::artifact_set_for_source(plan, &file_result.filename)
                else {
                    lifecycle
                        .fail(
                            &format!(
                                "{missing_artifact_label} job plan missing artifacts for {}",
                                file_result.filename
                            ),
                            FailureCategory::Validation,
                            unix_now(),
                        )
                        .await;
                    continue;
                };
                for artifact in &artifacts.files {
                    if artifact.role != MaterializedArtifactRole::Primary {
                        continue;
                    }
                    let output_text = if should_merge_abbrev {
                        apply_merge_abbrev_local(output_chat.as_ref())
                    } else {
                        output_chat.as_ref().to_string()
                    };
                    let target = crate::recipe_runner::runtime::ChatOutputTarget::new(
                        &job.filesystem,
                        file_index,
                        &artifact.display_path,
                    );
                    if let Err(error) = crate::recipe_runner::runtime::write_chat_output_artifact_with_provenance_gate(
                        &target,
                        &output_text,
                        job.dispatch.command,
                    )
                    .await
                    {
                        lifecycle
                            .fail(&error.to_string(), FailureCategory::System, unix_now())
                            .await;
                        continue;
                    }
                    lifecycle
                        .complete_with_result(
                            artifact.display_path.clone(),
                            artifact.content_type,
                            unix_now(),
                        )
                        .await;
                }
            }
            Err(error) => {
                lifecycle
                    .fail(
                        &error.to_string(),
                        FailureCategory::ProviderTerminal,
                        unix_now(),
                    )
                    .await;
            }
        }
    }
}

pub(crate) fn resolve_input_path(job: &RunnerJobSnapshot, file: &PendingJobFile) -> PathBuf {
    if job.filesystem.paths_mode && file.file_index < job.filesystem.source_paths.len() {
        job.filesystem.source_paths[file.file_index]
            .assume_shared_filesystem()
            .as_path()
            .to_owned()
    } else {
        job.filesystem
            .staging_dir
            .join("input")
            .join(file.filename.as_ref())
            .as_path()
            .to_owned()
    }
}

pub(crate) fn resolve_before_path(
    job: &RunnerJobSnapshot,
    file: &PendingJobFile,
) -> Option<PathBuf> {
    if !job.filesystem.before_paths.is_empty()
        && file.file_index < job.filesystem.before_paths.len()
    {
        Some(
            job.filesystem.before_paths[file.file_index]
                .assume_shared_filesystem()
                .as_path()
                .to_owned(),
        )
    } else {
        None
    }
}

fn apply_merge_abbrev_local(chat_text: &str) -> String {
    let parser = crate::chat_parser();
    let (mut file, _) = talkbank_transform::parse::parse_lenient(&parser, chat_text);
    talkbank_transform::merge_abbreviations(&mut file);
    talkbank_transform::serialize::to_chat_string(&file)
}
