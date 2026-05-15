use crate::common::{LiveServerJobClient, require_live_direct, require_live_server};
use crate::ml_golden::transcribe::helpers::{
    prepare_named_transcribe_fixture_job, transcribe_options,
};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{AsrEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

#[tokio::test]
async fn transcribe_direct_unsupported_asr_language_fails_cleanly() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };

    let Some(fixture) = prepare_named_transcribe_fixture_job(
        session.state_dir(),
        "transcribe_direct_unsupported_asr_language",
        "biling_vec_hrv_clip",
    ) else {
        return;
    };

    let submission = batchalign::api::JobSubmission {
        command: ReleasedCommand::Transcribe,
        lang: batchalign::api::LanguageSpec::try_from("vec").expect("valid vec language"),
        num_speakers: batchalign::api::NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Omit),
        paths_mode: true,
        source_paths: vec![fixture.source_path.as_str().into()],
        output_paths: vec![fixture.output_path.as_str().into()],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let (info, detail) = session.run_submission(submission).await;
    assert_eq!(
        info.status,
        JobStatus::Failed,
        "unsupported ASR language should fail cleanly in direct mode"
    );
    let error = detail
        .results
        .first()
        .and_then(|result| result.error.clone())
        .unwrap_or_default();
    assert!(
        error.contains("Unsupported language")
            || error.contains("unsupported language")
            || error.contains("venetian"),
        "direct unsupported-language failure should mention the language mismatch, got: {error}"
    );
}

#[tokio::test]
async fn transcribe_server_unsupported_asr_language_fails_cleanly() {
    let Some(server) =
        require_live_server(InferTask::Asr, "live server does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_named_transcribe_fixture_job(
        server.state_dir(),
        "transcribe_server_unsupported_asr_language",
        "biling_vec_hrv_clip",
    ) else {
        return;
    };

    let (info, _outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "vec",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Omit),
        )
        .await;

    assert_eq!(
        info.status,
        JobStatus::Failed,
        "unsupported ASR language should fail cleanly on the server path"
    );
    let results = jobs.job_results(&info.job_id).await;
    let error = results
        .files
        .first()
        .and_then(|result| result.error.clone())
        .unwrap_or_default();
    assert!(
        error.contains("Unsupported language")
            || error.contains("unsupported language")
            || error.contains("venetian"),
        "server unsupported-language failure should mention the language mismatch, got: {error}"
    );
}
