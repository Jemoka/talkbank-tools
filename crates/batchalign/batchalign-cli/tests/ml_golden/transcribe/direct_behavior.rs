use std::collections::BTreeSet;

use crate::common::{LiveDirectJobClient, assert_completed_without_errors, require_live_direct};
use crate::ml_golden::audio_helpers::{count_wor_tiers, parse_output};
use crate::ml_golden::transcribe::helpers::{
    prepare_multi_speaker_transcribe_fixture_job, prepare_transcribe_fixture_job,
    transcribe_options,
};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{AsrEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

fn media_header(chat: &str, label: &str) -> String {
    chat.lines()
        .find(|line| line.starts_with("@Media:\t"))
        .unwrap_or_else(|| panic!("{label}: expected @Media header"))
        .to_string()
}

#[tokio::test]
async fn direct_transcribe_produces_valid_chat() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) =
        prepare_transcribe_fixture_job(jobs.state_dir(), "direct_transcribe_verify")
    else {
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

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "direct_transcribe_verify should complete; error={:?}",
        info.error
    );
    assert_eq!(outputs.len(), 1);

    let file = parse_output(&outputs[0], "direct_transcribe_verify");
    assert!(
        file.utterance_count() >= 1,
        "direct_transcribe_verify: expected at least 1 utterance, got {}",
        file.utterance_count()
    );
}

#[tokio::test]
async fn direct_transcribe_wor_policy_controls_tier_presence() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_transcribe_fixture_job(
        jobs.state_dir(),
        "direct_transcribe_wor_policy_controls_tier_presence",
    ) else {
        return;
    };

    let (info_include, outputs_include) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path.clone()],
            vec![fixture.output_path.clone()],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Include),
        )
        .await;

    assert_completed_without_errors("direct_transcribe_wor_include", &info_include, &[]);
    assert_eq!(outputs_include.len(), 1);
    assert!(
        count_wor_tiers(&outputs_include[0]) > 0,
        "direct transcribe wor=Include should materialize %wor"
    );

    let (info_omit, outputs_omit) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("direct_transcribe_wor_omit", &info_omit, &[]);
    assert_eq!(outputs_omit.len(), 1);
    assert_eq!(
        count_wor_tiers(&outputs_omit[0]),
        0,
        "direct transcribe wor=Omit should suppress %wor"
    );
}

#[tokio::test]
async fn direct_transcribe_preserves_source_media_basename() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) =
        prepare_transcribe_fixture_job(jobs.state_dir(), "direct_transcribe_preserves_media_name")
    else {
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

    assert_completed_without_errors("direct_transcribe_preserves_media_name", &info, &[]);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        media_header(&outputs[0], "direct_transcribe_preserves_media_name"),
        "@Media:\ttest, audio",
        "transcribe should preserve the source media basename instead of a cached WAV name"
    );
}

#[tokio::test]
async fn direct_transcribe_diarize_surfaces_multiple_speakers_when_available() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    if !session.has_infer_task(InferTask::Speaker) {
        eprintln!("SKIP: direct session does not support speaker diarization");
        return;
    }
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_multi_speaker_transcribe_fixture_job(
        jobs.state_dir(),
        "direct_transcribe_diarize_surfaces_multiple_speakers_when_available",
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

    assert_completed_without_errors("direct_transcribe_diarize", &info, &[]);
    assert_eq!(outputs.len(), 1);
    let file = parse_output(&outputs[0], "direct_transcribe_diarize");
    let speakers: BTreeSet<String> = file
        .utterances()
        .map(|utt| utt.main.speaker.to_string())
        .collect();
    assert!(
        speakers.len() >= 2,
        "direct transcribe diarization should surface multiple speakers, got {:?}",
        speakers
    );
}
