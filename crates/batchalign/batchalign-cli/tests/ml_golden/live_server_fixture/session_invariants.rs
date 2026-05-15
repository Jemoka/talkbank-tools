use crate::common::{
    LiveServerJobClient, LiveServerSession, assert_completed_without_errors, chat_fixtures,
};
use batchalign::api::{FilePayload, LanguageCode3, LanguageSpec, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

/// The fixture should reuse the same warmed worker process across isolated sessions.
#[tokio::test]
async fn live_fixture_reuses_warmed_workers_across_sessions() {
    let Some(first) = LiveServerSession::acquire().await else {
        return;
    };
    if !first.has_infer_task(InferTask::Morphosyntax) {
        eprintln!("SKIP: live fixture does not support morphosyntax infer");
        return;
    }

    let first_health = first.health().await;
    assert!(
        first_health
            .loaded_pipelines
            .iter()
            .any(|pipeline| pipeline.contains("infer:morphosyntax:eng")),
        "expected the live fixture to pre-warm an English morphosyntax worker"
    );
    let first_pipelines = first_health.loaded_pipelines.clone();
    first.close().await;

    let Some(second) = LiveServerSession::acquire().await else {
        return;
    };
    if !second.has_infer_task(InferTask::Morphosyntax) {
        eprintln!("SKIP: live fixture does not support morphosyntax infer");
        return;
    }

    let second_health = second.health().await;
    let reused: Vec<&String> = second_health
        .loaded_pipelines
        .iter()
        .filter(|pipeline| first_pipelines.contains(*pipeline))
        .collect();
    assert!(
        !reused.is_empty(),
        "expected at least one warmed worker process to survive across isolated sessions"
    );
    second.close().await;
}

/// The fixture should provide a fresh runtime layout and empty job store each time.
#[tokio::test]
async fn live_fixture_isolates_runtime_state_between_sessions() {
    let Some(first) = LiveServerSession::acquire().await else {
        return;
    };
    if !first.has_infer_task(InferTask::Morphosyntax) {
        eprintln!("SKIP: live fixture does not support morphosyntax infer");
        return;
    }

    let first_state_dir = first.state_dir().to_path_buf();
    assert!(
        first_state_dir.join("jobs").exists(),
        "fixture session should own an explicit jobs directory"
    );
    let jobs = LiveServerJobClient::from_session(&first);
    let options = CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },

        ..Default::default()
    });

    let initial = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            vec![FilePayload {
                filename: "fixture-isolation.cha".into(),
                content: chat_fixtures::ENG_SIMPLE.into(),
            }],
            options,
        )
        .await;
    let final_info = jobs.poll_done(&initial.job_id).await;
    let results = jobs.job_results(&initial.job_id).await;
    assert_completed_without_errors("live_fixture_isolation", &final_info, &results.files);

    let first_jobs = first.list_jobs().await;
    assert_eq!(
        first_jobs.len(),
        1,
        "first session should see its submitted job"
    );
    first.close().await;

    let Some(second) = LiveServerSession::acquire().await else {
        return;
    };
    if !second.has_infer_task(InferTask::Morphosyntax) {
        eprintln!("SKIP: live fixture does not support morphosyntax infer");
        return;
    }

    assert_ne!(
        second.state_dir(),
        first_state_dir.as_path(),
        "each session should receive a fresh runtime-owned state directory"
    );
    let second_jobs = second.list_jobs().await;
    assert!(
        second_jobs.is_empty(),
        "fresh fixture session should start with an empty job listing"
    );
    let second_health = second.health().await;
    assert_eq!(
        second_health.active_jobs, 0,
        "fresh fixture session should not inherit active jobs"
    );
    second.close().await;
}
