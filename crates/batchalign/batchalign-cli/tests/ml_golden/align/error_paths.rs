use crate::common::{LiveServerJobClient, require_live_direct, require_live_server};
use batchalign::api::{
    JobStatus, JobSubmission, LanguageCode3, LanguageSpec, NumSpeakers, ReleasedCommand,
};
use batchalign::options::{AlignOptions, CommandOptions, CommonOptions};
use batchalign::worker::InferTask;

fn missing_audio_submission(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> JobSubmission {
    JobSubmission {
        command: ReleasedCommand::Align,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Align(AlignOptions {
            common: CommonOptions {
                override_media_cache: true,
                ..CommonOptions::default()
            },
            ..AlignOptions::default()
        }),
        paths_mode: true,
        source_paths: vec![input_path.to_string_lossy().as_ref().into()],
        output_paths: vec![output_path.to_string_lossy().as_ref().into()],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    }
}

fn write_missing_audio_chat(path: &std::path::Path) {
    std::fs::write(
        path,
        "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Media:\tnonexistent_audio, audio
*PAR:\thello world .
@End
",
    )
    .expect("write missing-audio input");
}

#[tokio::test]
async fn align_direct_missing_audio_fails_cleanly() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };

    let input_path = session.state_dir().join("align_missing_audio.cha");
    let output_path = session.state_dir().join("align_missing_audio_out.cha");
    write_missing_audio_chat(&input_path);

    let (final_info, _detail) = session
        .run_submission(missing_audio_submission(&input_path, &output_path))
        .await;
    assert_eq!(
        final_info.status,
        JobStatus::Failed,
        "align with missing audio should fail cleanly in direct mode"
    );
}

#[tokio::test]
async fn align_server_missing_audio_fails_cleanly() {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let input_path = server.state_dir().join("align_server_missing_audio.cha");
    let output_path = server
        .state_dir()
        .join("align_server_missing_audio_out.cha");
    write_missing_audio_chat(&input_path);

    let resp = jobs
        .post_json(
            "/jobs",
            &serde_json::to_value(missing_audio_submission(&input_path, &output_path))
                .expect("serialize missing-audio submission"),
        )
        .await;
    assert_eq!(
        resp.status(),
        200,
        "server missing-audio submission should succeed"
    );
    let initial = resp
        .json::<batchalign::api::JobInfo>()
        .await
        .expect("parse server missing-audio job info");
    let final_info = jobs.poll_done(&initial.job_id).await;
    assert_eq!(
        final_info.status,
        JobStatus::Failed,
        "align with missing audio should fail cleanly on the server path"
    );
}
