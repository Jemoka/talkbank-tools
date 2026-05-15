use crate::common::{LiveServerJobClient, assert_completed_without_errors, require_live_server};
use crate::ml_golden::align::helpers::{
    align_options, align_options_with_media_dir, prepare_align_fixture_job,
    prepare_align_media_dir_job,
};
use crate::ml_golden::audio_helpers::{assert_all_utterances_timed, count_wor_tiers};
use batchalign::api::ReleasedCommand;
use batchalign::options::{FaEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

fn first_main_line(chat: &str, label: &str) -> String {
    chat.lines()
        .find(|line| line.starts_with('*'))
        .unwrap_or_else(|| panic!("{label}: expected at least one main-tier line"))
        .to_string()
}

#[tokio::test]
async fn align_server_wor_policy_controls_tier_presence() {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_align_fixture_job(
        server.state_dir(),
        "align_server_wor_policy_controls_tier_presence",
    ) else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path.clone()],
            vec![fixture.output_path.clone()],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Include),
        )
        .await;

    assert_completed_without_errors("align_server_wor_include", &info, &[]);
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "align_server_wor_include");
    assert!(
        count_wor_tiers(&outputs[0]) > 0,
        "align server wor=Include should materialize %wor"
    );

    let (info_no_wor, outputs_no_wor) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("align_server_wor_omit", &info_no_wor, &[]);
    assert_eq!(outputs_no_wor.len(), 1);
    assert_all_utterances_timed(&outputs_no_wor[0], "align_server_wor_omit");
    assert_eq!(
        count_wor_tiers(&outputs_no_wor[0]),
        0,
        "align server wor=Omit should suppress %wor"
    );
}

#[tokio::test]
async fn align_server_before_preserves_existing_first_bullet() {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_align_fixture_job(
        server.state_dir(),
        "align_server_before_preserves_first_bullet",
    ) else {
        return;
    };

    let expected_first = first_main_line(
        &std::fs::read_to_string(&fixture.before_path).expect("read before CHAT"),
        "align_server_before_before_file",
    );

    let (info, outputs) = jobs
        .submit_paths_job_with_before(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            vec![fixture.before_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("align_server_before_preserves_first_bullet", &info, &[]);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        first_main_line(&outputs[0], "align_server_before_output"),
        expected_first,
        "server incremental align should preserve the first unchanged main-tier bullet"
    );
}

#[tokio::test]
async fn align_server_media_dir_recovers_audio_outside_adjacency() {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) =
        prepare_align_media_dir_job(server.state_dir(), "align_server_media_dir_recovers_audio")
    else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options_with_media_dir(
                FaEngineName::Wave2Vec,
                WorTierPolicy::Omit,
                Some(fixture.media_dir),
            ),
        )
        .await;

    assert_completed_without_errors("align_server_media_dir_recovers_audio", &info, &[]);
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "align_server_media_dir_recovers_audio");
}

#[tokio::test]
async fn align_server_without_media_dir_fails_when_audio_is_relocated() {
    let Some(server) =
        require_live_server(InferTask::Fa, "live server does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) =
        prepare_align_media_dir_job(server.state_dir(), "align_server_without_media_dir_fails")
    else {
        return;
    };

    let (info, _outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Omit),
        )
        .await;

    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Failed,
        "server align should fail when the adjacent audio is removed and no media_dir is provided"
    );
}
