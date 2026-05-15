use crate::common::{
    LiveDirectJobClient, assert_completed_without_errors, prepare_audio_fixtures,
    require_live_direct,
};
use crate::ml_golden::align::helpers::{
    align_options, align_options_with_media_dir, prepare_align_media_dir_job,
};
use crate::ml_golden::audio_helpers::assert_all_utterances_timed;
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{
    AlignOptions, CommandOptions, CommonOptions, FaEngineName, WorTierPolicy,
};
use batchalign::worker::InferTask;

#[tokio::test]
async fn option_align_wor_controls_tier_presence() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixtures) = prepare_audio_fixtures(jobs.state_dir()) else {
        return;
    };

    let out_include = jobs.state_dir().join("wor_include_out.cha");
    let (info_a, outputs_a) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixtures.stripped_chat.to_string_lossy().into()],
            vec![out_include.to_string_lossy().into()],
            CommandOptions::Align(AlignOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    ..CommonOptions::default()
                },
                fa_engine: FaEngineName::Wave2Vec,
                wor: WorTierPolicy::Include,
                ..AlignOptions::default()
            }),
        )
        .await;
    assert_completed_without_errors("wor_include", &info_a, &[]);

    let out_omit = jobs.state_dir().join("wor_omit_out.cha");
    let (info_b, outputs_b) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixtures.stripped_chat.to_string_lossy().into()],
            vec![out_omit.to_string_lossy().into()],
            CommandOptions::Align(AlignOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    ..CommonOptions::default()
                },
                fa_engine: FaEngineName::Wave2Vec,
                wor: WorTierPolicy::Omit,
                ..AlignOptions::default()
            }),
        )
        .await;
    assert_completed_without_errors("wor_omit", &info_b, &[]);

    let wor_count_include = outputs_a[0]
        .lines()
        .filter(|l| l.starts_with("%wor:"))
        .count();
    let wor_count_omit = outputs_b[0]
        .lines()
        .filter(|l| l.starts_with("%wor:"))
        .count();

    assert!(
        wor_count_include > 0,
        "wor=Include should produce %wor tiers"
    );
    assert_eq!(wor_count_omit, 0, "wor=Omit should produce no %wor tiers");
}

#[tokio::test]
async fn option_align_fa_engine_produces_different_timing() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixtures) = prepare_audio_fixtures(jobs.state_dir()) else {
        return;
    };

    let out_w2v = jobs.state_dir().join("fa_w2v_out.cha");
    let (info_a, outputs_a) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixtures.stripped_chat.to_string_lossy().into()],
            vec![out_w2v.to_string_lossy().into()],
            CommandOptions::Align(AlignOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    ..CommonOptions::default()
                },
                fa_engine: FaEngineName::Wave2Vec,
                wor: WorTierPolicy::Omit,
                ..AlignOptions::default()
            }),
        )
        .await;
    assert_completed_without_errors("fa_wav2vec", &info_a, &[]);

    let out_wh = jobs.state_dir().join("fa_whisper_out.cha");
    let (info_b, outputs_b) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixtures.stripped_chat.to_string_lossy().into()],
            vec![out_wh.to_string_lossy().into()],
            CommandOptions::Align(AlignOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    ..CommonOptions::default()
                },
                fa_engine: FaEngineName::Whisper,
                wor: WorTierPolicy::Omit,
                ..AlignOptions::default()
            }),
        )
        .await;
    assert_completed_without_errors("fa_whisper", &info_b, &[]);

    assert_ne!(
        outputs_a[0], outputs_b[0],
        "Wave2Vec and Whisper FA should produce different timing"
    );
}

#[tokio::test]
async fn option_align_media_dir_recovers_audio_outside_adjacency() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) =
        prepare_align_media_dir_job(jobs.state_dir(), "align_media_dir_recovers_audio")
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

    assert_completed_without_errors("align_media_dir_recovers_audio", &info, &[]);
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(outputs.len(), 1);
    assert_all_utterances_timed(&outputs[0], "align_media_dir_recovers_audio");
}

#[tokio::test]
async fn option_align_without_media_dir_fails_when_audio_is_relocated() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) =
        prepare_align_media_dir_job(jobs.state_dir(), "align_without_media_dir_fails")
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
        JobStatus::Failed,
        "align should fail when the adjacent audio is removed and no media_dir is provided"
    );
}
