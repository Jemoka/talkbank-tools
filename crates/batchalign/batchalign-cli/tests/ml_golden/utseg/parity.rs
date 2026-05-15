use crate::ml_golden::parity_helpers::run_parity_test;
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, UtsegOptions};
use batchalign::worker::InferTask;

fn utseg_opts() -> CommandOptions {
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
async fn parity_utseg_eng_multi() {
    run_parity_test(
        ReleasedCommand::Utseg,
        InferTask::Utseg,
        "eng_multi_speaker",
        "eng",
        utseg_opts(),
    )
    .await;
}

#[tokio::test]
async fn parity_utseg_spa() {
    run_parity_test(
        ReleasedCommand::Utseg,
        InferTask::Utseg,
        "spa_simple",
        "spa",
        utseg_opts(),
    )
    .await;
}

#[tokio::test]
async fn parity_utseg_eng_disfluency() {
    run_parity_test(
        ReleasedCommand::Utseg,
        InferTask::Utseg,
        "eng_disfluency",
        "eng",
        utseg_opts(),
    )
    .await;
}
