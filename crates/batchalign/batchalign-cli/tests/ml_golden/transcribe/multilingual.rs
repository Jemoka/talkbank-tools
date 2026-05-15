use crate::common::{LiveServerJobClient, require_live_server};
use crate::ml_golden::audio_helpers::{parse_output, transcribe_audio_clip};
use crate::ml_golden::transcribe::helpers::{
    prepare_named_transcribe_fixture_job, transcribe_options,
};
use batchalign::api::ReleasedCommand;
use batchalign::options::{AsrEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

#[tokio::test]
async fn transcribe_spa_whisper() {
    transcribe_audio_clip("spa_marrero_clip", "spa", "transcribe_spa").await;
}

#[tokio::test]
async fn transcribe_fra_whisper() {
    transcribe_audio_clip("fra_geneva_clip", "fra", "transcribe_fra").await;
}

#[tokio::test]
async fn transcribe_jpn_whisper() {
    transcribe_audio_clip("jpn_tyo_clip", "jpn", "transcribe_jpn").await;
}

#[tokio::test]
async fn transcribe_yue_whisper() {
    transcribe_audio_clip("yue_hku_clip", "yue", "transcribe_yue").await;
}

#[tokio::test]
async fn transcribe_biling_cat_spa_whisper() {
    transcribe_audio_clip("biling_cat_spa_clip", "cat", "transcribe_biling_cat_spa").await;
}

#[tokio::test]
async fn transcribe_eng_multi_speaker_whisper() {
    transcribe_audio_clip("eng_multi_speaker", "eng", "transcribe_eng_multi_speaker").await;
}

async fn transcribe_audio_clip_server(audio_name: &str, lang: &str, label: &str) {
    let Some(server) =
        require_live_server(InferTask::Asr, "live server does not support ASR infer").await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let Some(fixture) = prepare_named_transcribe_fixture_job(server.state_dir(), label, audio_name)
    else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Transcribe,
            lang,
            vec![fixture.source_path],
            vec![fixture.output_path],
            transcribe_options(AsrEngineName::Whisper, false, WorTierPolicy::Omit),
        )
        .await;

    if info.status != batchalign::api::JobStatus::Completed {
        let results = jobs.job_results(&info.job_id).await;
        panic!("{label}: server job failed; info={info:?}; results={results:#?}");
    }
    assert_eq!(
        info.status,
        batchalign::api::JobStatus::Completed,
        "{label}: server job should complete; error={:?}",
        info.error
    );
    assert_eq!(outputs.len(), 1);
    let file = parse_output(&outputs[0], label);
    assert!(
        file.utterance_count() >= 1,
        "{label}: expected at least 1 utterance, got {}",
        file.utterance_count()
    );
}

#[tokio::test]
async fn transcribe_server_spa_whisper() {
    transcribe_audio_clip_server("spa_marrero_clip", "spa", "transcribe_server_spa").await;
}

#[tokio::test]
async fn transcribe_server_yue_whisper() {
    transcribe_audio_clip_server("yue_hku_clip", "yue", "transcribe_server_yue").await;
}

#[tokio::test]
async fn transcribe_server_biling_cat_spa_whisper() {
    transcribe_audio_clip_server(
        "biling_cat_spa_clip",
        "cat",
        "transcribe_server_biling_cat_spa",
    )
    .await;
}
