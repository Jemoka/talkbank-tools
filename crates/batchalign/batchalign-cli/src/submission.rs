//! Build internal jobs from validated API submissions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use batchalign_types::paths::ServerPath;
use tokio_util::sync::CancellationToken;

use crate::api::{CorrelationId, DisplayPath, JobId, JobStatus, JobSubmission};
use crate::error::ServerError;
use crate::store::{
    FileStatus, Job, JobDispatchConfig, JobExecutionState, JobFilesystemConfig, JobIdentity,
    JobLeaseState, JobRuntimeControl, JobScheduleState, JobSourceContext, unix_now,
};

/// Trusted host-side context needed to materialize one job from a submission.
pub(crate) struct SubmissionContext {
    /// Stable identifier for the new job.
    pub job_id: JobId,
    /// Correlation context attached to logs and responses.
    pub correlation_id: CorrelationId,
    /// Host-owned runtime jobs directory.
    pub jobs_dir: PathBuf,
    /// Submitter identity used for conflict detection and display.
    pub submitted_by: String,
    /// Human-readable submitter name used for display.
    pub submitted_by_name: String,
}

fn path_mode_filename_from_source(source_path: &str) -> Result<DisplayPath, ServerError> {
    let file_name = Path::new(source_path)
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            ServerError::Validation(format!(
                "paths_mode source path has no filename component: {source_path}"
            ))
        })?;
    Ok(DisplayPath::from(file_name.to_string_lossy().to_string()))
}

async fn ensure_dir(path: &Path, context: &str) -> Result<(), ServerError> {
    tokio::fs::create_dir_all(path).await.map_err(|error| {
        ServerError::Io(std::io::Error::new(
            error.kind(),
            format!("{context} {}: {error}", path.display()),
        ))
    })
}

async fn write_staged_file(path: &Path, content: &str) -> Result<(), ServerError> {
    tokio::fs::write(path, content).await.map_err(|error| {
        ServerError::Io(std::io::Error::new(
            error.kind(),
            format!("staging input file {}: {error}", path.display()),
        ))
    })
}

/// Materialize one internal job from a validated API submission.
pub(crate) async fn materialize_submission_job(
    submission: &JobSubmission,
    context: &SubmissionContext,
) -> Result<Job, ServerError> {
    submission
        .validate()
        .map_err(|error| ServerError::Validation(error.to_string()))?;

    let staging_dir = ServerPath::from(context.jobs_dir.join(context.job_id.to_string()));
    let (filenames, has_chat, paths_mode, source_paths, output_paths) = if submission.paths_mode {
        let filenames: Vec<DisplayPath> = if !submission.display_names.is_empty() {
            submission
                .display_names
                .iter()
                .map(|name| DisplayPath::from(name.as_str()))
                .collect()
        } else {
            submission
                .source_paths
                .iter()
                .map(|path| path_mode_filename_from_source(path.as_str()))
                .collect::<Result<Vec<_>, _>>()?
        };
        let has_chat: Vec<bool> = submission
            .source_paths
            .iter()
            .map(|path| path.as_str().to_ascii_lowercase().ends_with(".cha"))
            .collect();

        ensure_dir(staging_dir.as_ref(), "creating paths-mode staging dir").await?;
        ensure_dir(
            staging_dir.join("output").as_ref(),
            "creating paths-mode staged output dir",
        )
        .await?;

        (
            filenames,
            has_chat,
            true,
            submission.source_paths.clone(),
            submission.output_paths.clone(),
        )
    } else {
        if submission.files.is_empty() && submission.media_files.is_empty() {
            return Err(ServerError::Validation(
                "Must provide at least one file or media_file.".into(),
            ));
        }

        let input_dir = staging_dir.join("input");
        ensure_dir(input_dir.as_ref(), "creating content-mode input dir").await?;
        ensure_dir(
            staging_dir.join("output").as_ref(),
            "creating content-mode output dir",
        )
        .await?;

        let mut filenames: Vec<DisplayPath> = Vec::new();
        let mut has_chat = Vec::new();

        for file in &submission.files {
            filenames.push(file.filename.clone());
            has_chat.push(true);
            let dest = input_dir.as_path().join(file.filename.as_ref());
            if let Some(parent) = dest.parent() {
                ensure_dir(parent, "creating staged input parent dir").await?;
            }
            write_staged_file(&dest, &file.content).await?;
        }
        for media_name in &submission.media_files {
            filenames.push(DisplayPath::from(media_name.as_str()));
            has_chat.push(false);
        }

        (filenames, has_chat, false, Vec::new(), Vec::new())
    };

    let mut file_statuses = HashMap::new();
    for filename in &filenames {
        file_statuses.insert(filename.to_string(), FileStatus::new(filename.clone()));
    }

    Ok(Job {
        identity: JobIdentity {
            job_id: context.job_id.clone(),
            correlation_id: context.correlation_id.clone(),
        },
        dispatch: JobDispatchConfig {
            command: submission.command,
            lang: submission.lang.clone(),
            num_speakers: submission.num_speakers,
            options: submission.options.clone(),
            runtime_state: std::collections::BTreeMap::new(),
            debug_traces: submission.debug_traces,
        },
        source: JobSourceContext {
            submitted_by: context.submitted_by.clone(),
            submitted_by_name: context.submitted_by_name.clone(),
            source_dir: submission.source_dir.clone(),
        },
        filesystem: JobFilesystemConfig {
            filenames,
            has_chat,
            staging_dir,
            paths_mode,
            source_paths,
            output_paths,
            before_paths: if submission.paths_mode {
                submission.before_paths.clone()
            } else {
                Vec::new()
            },
            media_mapping: submission.media_mapping.clone(),
            media_subdir: submission.media_subdir.clone(),
            source_dir: submission.source_dir.clone(),
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
    })
}

#[cfg(test)]
mod tests {
    use super::{SubmissionContext, materialize_submission_job};
    use crate::api::{
        DisplayPath, FilePayload, JobSubmission, LanguageSpec, NumSpeakers, ReleasedCommand,
    };
    use crate::options::{CommandOptions, CommonOptions, MorphotagOptions};

    fn morphotag_submission(paths_mode: bool) -> JobSubmission {
        // Morphotag has no job-level `--lang`; per-file resolution from
        // each file's `@Languages:` header is mandatory. Job-level lang
        // sentinels are rejected by `materialize_submission_job` (see the
        // 2026-05-03 morphotag incident note in `validate_submission`).
        JobSubmission {
            command: ReleasedCommand::Morphotag,
            lang: LanguageSpec::PerFile,
            num_speakers: NumSpeakers(1),
            files: Vec::new(),
            media_files: Vec::new(),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: "/tmp/input".into(),
            options: CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions::default(),

                ..Default::default()
            }),
            paths_mode,
            source_paths: Vec::new(),
            output_paths: Vec::new(),
            display_names: Vec::new(),
            debug_traces: false,
            before_paths: Vec::new(),
        }
    }

    #[tokio::test]
    async fn materialize_paths_submission_uses_display_names() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut submission = morphotag_submission(true);
        submission.source_paths = vec!["/tmp/source/sample.cha".into()];
        submission.output_paths = vec!["/tmp/output/sample.cha".into()];
        submission.display_names = vec!["nested/sample.cha".into()];

        let context = SubmissionContext {
            job_id: "job-paths".into(),
            correlation_id: "corr-paths".into(),
            jobs_dir: tempdir.path().join("jobs"),
            submitted_by: "127.0.0.1".into(),
            submitted_by_name: "localhost".into(),
        };

        let job = materialize_submission_job(&submission, &context)
            .await
            .expect("materialize paths submission");

        assert!(job.filesystem.paths_mode);
        assert_eq!(
            job.filesystem.filenames,
            vec![DisplayPath::from("nested/sample.cha")]
        );
        assert_eq!(
            job.filesystem.source_paths[0],
            batchalign_types::paths::ClientPath::new("/tmp/source/sample.cha")
        );
        assert_eq!(
            job.filesystem.output_paths[0],
            batchalign_types::paths::ClientPath::new("/tmp/output/sample.cha")
        );
        assert!(job.filesystem.staging_dir.as_path().ends_with("job-paths"));
        assert!(job.filesystem.staging_dir.as_path().exists());
    }

    #[tokio::test]
    async fn materialize_content_submission_stages_chat_inputs() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let mut submission = morphotag_submission(false);
        submission.files = vec![FilePayload {
            filename: DisplayPath::from("nested/sample.cha"),
            content: "@UTF8\n@Begin\n*CHI:\tI go .\n@End\n".into(),
        }];

        let context = SubmissionContext {
            job_id: "job-content".into(),
            correlation_id: "corr-content".into(),
            jobs_dir: tempdir.path().join("jobs"),
            submitted_by: "127.0.0.1".into(),
            submitted_by_name: "localhost".into(),
        };

        let job = materialize_submission_job(&submission, &context)
            .await
            .expect("materialize content submission");

        let staged_input = job.filesystem.staging_dir.join("input/nested/sample.cha");
        assert!(!job.filesystem.paths_mode);
        assert!(job.filesystem.staging_dir.join("output").as_path().exists());
        assert_eq!(
            tokio::fs::read_to_string(staged_input)
                .await
                .expect("read staged file"),
            "@UTF8\n@Begin\n*CHI:\tI go .\n@End\n"
        );
    }
}
