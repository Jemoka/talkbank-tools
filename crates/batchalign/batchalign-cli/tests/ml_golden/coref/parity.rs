use crate::ml_golden::parity_helpers::run_parity_test;
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, CorefOptions};
use batchalign::worker::InferTask;

fn coref_opts() -> CommandOptions {
    CommandOptions::Coref(CorefOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    })
}

#[tokio::test]
async fn parity_coref_eng_disfluency() {
    run_parity_test(
        ReleasedCommand::Coref,
        InferTask::Coref,
        "eng_disfluency",
        "eng",
        coref_opts(),
    )
    .await;
}

#[tokio::test]
async fn parity_coref_eng_multi_speaker() {
    run_parity_test(
        ReleasedCommand::Coref,
        InferTask::Coref,
        "eng_multi_speaker",
        "eng",
        coref_opts(),
    )
    .await;
}
