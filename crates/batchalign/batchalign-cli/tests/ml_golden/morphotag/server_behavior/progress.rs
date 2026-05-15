use super::super::helpers::repeated_chat;
use crate::common::{LiveServerJobClient, require_live_server};
use batchalign::api::{
    FilePayload, FileProgressStage, JobStatus, LanguageCode3, LanguageSpec, ReleasedCommand,
};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

/// Verify the new morphotag architecture still surfaces per-language batch
/// progress for mixed-language jobs rather than collapsing everything onto one
/// generic group.
#[tokio::test]
async fn morphotag_multilingual_job_reports_separate_batch_progress_groups() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let initial = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            vec![
                FilePayload {
                    filename: "english.cha".into(),
                    content: repeated_chat("eng", "ENG", "the dog runs", 220),
                },
                FilePayload {
                    filename: "spanish.cha".into(),
                    content: repeated_chat("spa", "SPA", "el perro corre", 220),
                },
            ],
            CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    batch_window: 0,
                    ..CommonOptions::default()
                },

                ..Default::default()
            }),
        )
        .await;

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(180);
    let mut observed_groups = std::collections::BTreeSet::new();

    loop {
        let info = jobs.job_info(&initial.job_id).await;
        if let Some(progress) = &info.batch_progress {
            observed_groups.extend(progress.language_groups.keys().cloned());
        }
        if info.status.is_terminal() || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    let final_info = jobs.poll_done(&initial.job_id).await;
    assert_eq!(
        final_info.status,
        JobStatus::Completed,
        "multilingual morphotag job should complete"
    );
    assert!(
        observed_groups.contains("eng"),
        "expected eng batch progress group, observed {observed_groups:?}"
    );
    assert!(
        observed_groups.contains("spa"),
        "expected spa batch progress group, observed {observed_groups:?}"
    );
}

/// Verify batch-window progress is visible on the server path so operators can
/// tell a multi-window morphotag job is advancing rather than appearing stuck.
#[tokio::test]
async fn morphotag_windowed_job_reports_file_window_progress() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let files: Vec<FilePayload> = (0..4)
        .map(|idx| FilePayload {
            filename: format!("window_{idx}.cha").into(),
            content: repeated_chat("eng", "PAR", "the window test runs", 180),
        })
        .collect();

    let initial = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            files,
            CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    batch_window: 1,
                    ..CommonOptions::default()
                },

                ..Default::default()
            }),
        )
        .await;

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(180);
    let mut max_progress_total = 0;
    let mut max_progress_current = 0;

    loop {
        let info = jobs.job_info(&initial.job_id).await;
        for file in &info.file_statuses {
            if file.progress_stage == Some(FileProgressStage::Parsing) {
                max_progress_total = max_progress_total.max(file.progress_total.unwrap_or(0));
                max_progress_current = max_progress_current.max(file.progress_current.unwrap_or(0));
            }
        }
        if info.status.is_terminal() || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    let final_info = jobs.poll_done(&initial.job_id).await;
    assert_eq!(
        final_info.status,
        JobStatus::Completed,
        "windowed morphotag job should complete"
    );
    assert!(
        max_progress_total >= 4,
        "expected multi-window total progress, observed total={max_progress_total}"
    );
    assert!(
        max_progress_current >= 2,
        "expected window progress to advance beyond the first window, observed current={max_progress_current}"
    );
}
