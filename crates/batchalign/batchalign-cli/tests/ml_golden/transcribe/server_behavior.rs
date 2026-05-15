use std::collections::BTreeSet;

use crate::common::{LiveServerJobClient, assert_completed_without_errors, require_live_server};
use crate::ml_golden::audio_helpers::{count_wor_tiers, parse_output};
use crate::ml_golden::transcribe::helpers::{
    prepare_multi_speaker_transcribe_fixture_job, prepare_transcribe_fixture_job,
    transcribe_options,
};
use batchalign::api::ReleasedCommand;
use batchalign::options::{AsrEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

fn media_header(chat: &str, label: &str) -> String {
    chat.lines()
        .find(|line| line.starts_with("@Media:\t"))
        .unwrap_or_else(|| panic!("{label}: expected @Media header"))
        .to_string()
}

#[tokio::test]
async fn transcribe_server_wor_policy_controls_tier_presence() {
    let Some(server) =
        require_live_server(InferTask::Asr, "live server does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_transcribe_fixture_job(
        server.state_dir(),
        "transcribe_server_wor_policy_controls_tier_presence",
    ) else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path.clone()],
            vec![fixture.output_path.clone()],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Include),
        )
        .await;

    assert_completed_without_errors("transcribe_server_wor_include", &info, &[]);
    assert_eq!(outputs.len(), 1);
    assert!(
        count_wor_tiers(&outputs[0]) > 0,
        "transcribe server wor=Include should materialize %wor"
    );

    let (info_no_wor, outputs_no_wor) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("transcribe_server_wor_omit", &info_no_wor, &[]);
    assert_eq!(outputs_no_wor.len(), 1);
    assert_eq!(
        count_wor_tiers(&outputs_no_wor[0]),
        0,
        "transcribe server wor=Omit should suppress %wor"
    );
}

#[tokio::test]
async fn transcribe_server_diarize_surfaces_multiple_speakers_when_available() {
    let Some(server) =
        require_live_server(InferTask::Asr, "live server does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);
    if !server.has_infer_task(InferTask::Speaker) {
        eprintln!("SKIP: live server does not support speaker diarization");
        return;
    }

    let Some(fixture) = prepare_multi_speaker_transcribe_fixture_job(
        server.state_dir(),
        "transcribe_server_diarize_surfaces_multiple_speakers_when_available",
    ) else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, true, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("transcribe_server_diarize", &info, &[]);
    assert_eq!(outputs.len(), 1);

    let file = parse_output(&outputs[0], "transcribe_server_diarize");
    let speakers: BTreeSet<String> = file
        .utterances()
        .map(|utt| utt.main.speaker.to_string())
        .collect();
    assert!(
        speakers.len() >= 2,
        "transcribe server diarization should surface multiple speakers, got {:?}",
        speakers
    );
}

#[tokio::test]
async fn transcribe_server_preserves_source_media_basename() {
    let Some(server) =
        require_live_server(InferTask::Asr, "live server does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_transcribe_fixture_job(
        server.state_dir(),
        "transcribe_server_preserves_source_media_basename",
    ) else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("transcribe_server_preserves_media_name", &info, &[]);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        media_header(&outputs[0], "transcribe_server_preserves_media_name"),
        "@Media:\ttest, audio",
        "server transcribe should preserve the source media basename instead of a cached WAV name"
    );
}
