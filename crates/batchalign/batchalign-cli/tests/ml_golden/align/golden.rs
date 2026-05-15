use crate::common::{LiveDirectJobClient, assert_completed_without_errors, require_live_direct};
use crate::ml_golden::align::helpers::{align_options, prepare_align_fixture_job};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{FaEngineName, WorTierPolicy};
use batchalign::worker::InferTask;

use crate::ml_golden::audio_helpers::{
    assert_all_utterances_timed, assert_valid_chat_structure, count_wor_tiers,
};

#[tokio::test]
async fn golden_align_eng_wav2vec() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_align_fixture_job(jobs.state_dir(), "align_wav2vec") else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Include),
        )
        .await;

    assert_completed_without_errors("align_eng_wav2vec", &info, &[]);
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(outputs.len(), 1);

    let output = &outputs[0];
    assert_all_utterances_timed(output, "align_eng_wav2vec");
    assert!(
        count_wor_tiers(output) > 0,
        "align_eng_wav2vec: %wor tier should be present"
    );
    assert_valid_chat_structure(output, "align_eng_wav2vec");
}

#[tokio::test]
async fn golden_align_eng_whisper_fa() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_align_fixture_job(jobs.state_dir(), "align_whisper") else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options(FaEngineName::Whisper, WorTierPolicy::Include),
        )
        .await;

    assert_completed_without_errors("align_eng_whisper_fa", &info, &[]);
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(outputs.len(), 1);

    let output = &outputs[0];
    assert_all_utterances_timed(output, "align_eng_whisper_fa");
    assert!(
        count_wor_tiers(output) > 0,
        "align_eng_whisper_fa: %wor tier should be present"
    );
    assert_valid_chat_structure(output, "align_eng_whisper_fa");
}

#[tokio::test]
async fn golden_align_eng_no_wor() {
    let Some(session) =
        require_live_direct(InferTask::Fa, "Direct session does not support FA infer").await
    else {
        return;
    };
    let jobs = LiveDirectJobClient::new(&session);

    let Some(fixture) = prepare_align_fixture_job(jobs.state_dir(), "align_no_wor") else {
        return;
    };

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            "eng",
            vec![fixture.source_path],
            vec![fixture.output_path],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Omit),
        )
        .await;

    assert_completed_without_errors("align_eng_no_wor", &info, &[]);
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(outputs.len(), 1);

    let output = &outputs[0];
    assert_all_utterances_timed(output, "align_eng_no_wor");
    assert_eq!(
        count_wor_tiers(output),
        0,
        "align_eng_no_wor: %wor tier should be absent when wor=Omit"
    );
    assert_valid_chat_structure(output, "align_eng_no_wor");
}
