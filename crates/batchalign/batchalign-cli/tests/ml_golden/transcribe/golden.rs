use crate::common::{LiveDirectJobClient, require_live_direct};
use crate::ml_golden::audio_helpers::{
    assert_first_utterance_max_words, assert_min_utterances, assert_valid_chat_structure,
    count_wor_tiers, revai_available,
};
use crate::ml_golden::transcribe::helpers::{
    prepare_named_transcribe_fixture_job, prepare_transcribe_fixture_job, transcribe_options,
};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{AsrEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

#[tokio::test]
async fn golden_transcribe_eng_whisper() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_transcribe_fixture_job(jobs.state_dir(), "transcribe_whisper")
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
        "transcribe_eng_whisper: job should complete"
    );
    assert_eq!(outputs.len(), 1);

    let output = &outputs[0];
    assert_valid_chat_structure(output, "transcribe_eng_whisper");
    assert!(
        output.contains("eng"),
        "transcribe_eng_whisper: output should reference English language"
    );
}

#[tokio::test]
async fn golden_transcribe_eng_revai() {
    if !revai_available() {
        return;
    }

    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_transcribe_fixture_job(jobs.state_dir(), "transcribe_revai") else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::RevAi, false, WorTierPolicy::Omit),
        )
        .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "transcribe_eng_revai: job should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_valid_chat_structure(&outputs[0], "transcribe_eng_revai");
}

#[tokio::test]
async fn golden_transcribe_eng_whisper_wor() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_transcribe_fixture_job(jobs.state_dir(), "transcribe_wor") else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Include),
        )
        .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "transcribe_eng_whisper_wor: job should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_valid_chat_structure(&outputs[0], "transcribe_eng_whisper_wor");
    assert!(
        count_wor_tiers(&outputs[0]) > 0,
        "transcribe_eng_whisper_wor: %wor tier should be present"
    );
}

#[tokio::test]
async fn transcribe_eng_acr_clip_whisper_produces_multiple_utterances() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_named_transcribe_fixture_job(
        jobs.state_dir(),
        "transcribe_eng_acr_clip",
        "eng_acr_first13p5",
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

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "transcribe_eng_acr_clip: job should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_valid_chat_structure(&outputs[0], "transcribe_eng_acr_clip");
    assert_min_utterances(&outputs[0], "transcribe_eng_acr_clip", 3);
}

#[tokio::test]
async fn transcribe_eng_acr_clip_revai_avoids_giant_first_utterance() {
    if !revai_available() {
        return;
    }

    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_named_transcribe_fixture_job(
        jobs.state_dir(),
        "transcribe_eng_acr_clip_revai",
        "eng_acr_first13p5",
    ) else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::RevAi, false, WorTierPolicy::Omit),
        )
        .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "transcribe_eng_acr_clip_revai: job should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_valid_chat_structure(&outputs[0], "transcribe_eng_acr_clip_revai");
    assert_min_utterances(&outputs[0], "transcribe_eng_acr_clip_revai", 4);
    assert_first_utterance_max_words(&outputs[0], "transcribe_eng_acr_clip_revai", 6);
}

#[tokio::test]
async fn transcribe_eng_diarize() {
    let Some(session) =
        require_live_direct(InferTask::Asr, "Direct session does not support ASR infer").await
    else {
        return;
    };
    if !session.has_infer_task(InferTask::Speaker) {
        eprintln!("SKIP: Direct session does not support speaker diarization");
        return;
    }
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_transcribe_fixture_job(jobs.state_dir(), "transcribe_diarize")
    else {
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

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "transcribe_eng_diarize: job should complete"
    );
    assert_eq!(outputs.len(), 1);
    assert_valid_chat_structure(&outputs[0], "transcribe_eng_diarize");
}
