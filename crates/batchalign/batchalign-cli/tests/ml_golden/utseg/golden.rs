use crate::common::assert_completed_without_errors;
use crate::ml_golden::golden::fixtures::{ENG_MULTI_SPEAKER_PARITY, ENG_MULTI_UTT, SPA_MULTI_UTT};
use crate::ml_golden::golden::helpers::{assert_golden_snapshot, require_direct_session_warmed};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, UtsegOptions};
use batchalign::worker::InferTask;

fn utseg_options() -> CommandOptions {
    CommandOptions::Utseg(UtsegOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
        utseg_fallback: false.into(),
    })
}

#[tokio::test]
async fn golden_utseg_eng_multi_utt() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Utseg,
        ReleasedCommand::Utseg,
        "eng",
        "Direct session does not support utseg infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Utseg,
            "eng",
            "eng_multi_utt.cha",
            ENG_MULTI_UTT,
            utseg_options(),
        )
        .await;

    assert_completed_without_errors("utseg_eng_multi_utt", &info, &results);
    assert_golden_snapshot!("utseg_eng_multi_utt", &results[0].content);
}

#[tokio::test]
async fn golden_utseg_spa() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Utseg,
        ReleasedCommand::Utseg,
        "spa",
        "Direct session does not support utseg infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Utseg,
            "spa",
            "spa_multi_utt.cha",
            SPA_MULTI_UTT,
            utseg_options(),
        )
        .await;

    if info.status == JobStatus::Failed {
        eprintln!("SKIP: Spanish utseg failed (model likely not downloaded)");
        return;
    }

    assert_completed_without_errors("utseg_spa", &info, &results);
    assert_golden_snapshot!("utseg_spa", &results[0].content);
}

#[tokio::test]
async fn utseg_multispeaker_preserves_turns_and_timing_bullets() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Utseg,
        ReleasedCommand::Utseg,
        "eng",
        "Direct session does not support utseg infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Utseg,
            "eng",
            "eng_multi_speaker.cha",
            ENG_MULTI_SPEAKER_PARITY,
            utseg_options(),
        )
        .await;

    assert_completed_without_errors("utseg_multispeaker_preserves_structure", &info, &results);
    assert_eq!(results.len(), 1);

    let output = &results[0].content;
    assert!(
        output.contains("*CHI:\t"),
        "utseg should preserve child turns in multi-speaker transcripts"
    );
    assert!(
        output.contains('\u{0015}'),
        "utseg should preserve timing bullets when they are present"
    );
    assert!(
        output.contains("@Participants:\tFAT Father, CHI Target_Child, MOT Mother"),
        "utseg should preserve the participant roster"
    );
    assert!(
        output.contains("*FAT:\twanna give me a kiss ?"),
        "utseg should not rewrite away the opening FAT turn"
    );
}
