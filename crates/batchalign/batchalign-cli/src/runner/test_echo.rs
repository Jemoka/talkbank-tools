//! Test-echo dispatch: reads input files and writes them back as output,
//! exercising the full file lifecycle (progress tracking, result recording)
//! without any ML inference. Used by worker integration tests.

use crate::api::ContentType;
use crate::recipe_runner::runtime::{
    ChatOutputTarget, result_display_path_for_command, write_text_output_artifact,
};
use crate::scheduling::{FailureCategory, WorkUnitKind};
use crate::store::{RunnerJobSnapshot, unix_now};

use super::util::{FileRunTracker, FileStage, RunnerEventSink};

/// Dispatch each file in test-echo mode: read input, write it back as output.
pub(super) async fn dispatch_test_echo_files(
    job: &RunnerJobSnapshot,
    sink: &dyn RunnerEventSink,
    file_list: &[crate::store::PendingJobFile],
) {
    let job_id = &job.identity.job_id;

    for file in file_list {
        if job.cancel_token.is_cancelled() {
            break;
        }

        let filename = file.filename.as_ref();
        let lifecycle = FileRunTracker::new(sink, job_id, filename);
        let started_at = unix_now();
        lifecycle
            .begin_first_attempt(WorkUnitKind::FileProcess, started_at, FileStage::Processing)
            .await;

        let result_display_path = result_display_path_for_command(job.dispatch.command, filename);

        let output_text = if file.has_chat {
            let read_path: std::path::PathBuf = if job.filesystem.paths_mode
                && file.file_index < job.filesystem.source_paths.len()
            {
                job.filesystem.source_paths[file.file_index]
                    .assume_shared_filesystem()
                    .as_path()
                    .to_owned()
            } else {
                job.filesystem
                    .staging_dir
                    .join("input")
                    .join(filename)
                    .as_path()
                    .to_owned()
            };
            match tokio::fs::read_to_string(&read_path).await {
                Ok(content) => content,
                Err(error) => {
                    let err_msg = format!("Failed to read input for test-echo dispatch: {error}");
                    lifecycle
                        .fail(&err_msg, FailureCategory::InputMissing, unix_now())
                        .await;
                    continue;
                }
            }
        } else {
            "@UTF8\n@Begin\n@End\n".to_string()
        };

        let target = ChatOutputTarget::new(&job.filesystem, file.file_index, &result_display_path);
        if let Err(error) = write_text_output_artifact(&target, &output_text).await {
            let err_msg = format!("Failed to write test-echo output: {error}");
            lifecycle
                .fail(&err_msg, FailureCategory::Validation, unix_now())
                .await;
            continue;
        }

        lifecycle
            .complete_with_result(result_display_path, ContentType::Chat, unix_now())
            .await;
    }
}
