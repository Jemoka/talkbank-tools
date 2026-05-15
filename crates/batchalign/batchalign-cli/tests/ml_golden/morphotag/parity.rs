use crate::ml_golden::parity_helpers::run_parity_test;
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

fn morphotag_opts(retokenize: bool) -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        retokenize,

        ..Default::default()
    })
}

#[tokio::test]
async fn parity_morphotag_eng_disfluency() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "eng_disfluency",
        "eng",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_eng_multi_speaker() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "eng_multi_speaker",
        "eng",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_eng_retokenize() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "eng_retokenize",
        "eng",
        morphotag_opts(true),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_eng_overlap() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "eng_overlap_ca",
        "eng",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_spa() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "spa_simple",
        "spa",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_fra() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "fra_simple",
        "fra",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_deu() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "deu_clinical",
        "deu",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_jpn() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "jpn_clinical",
        "jpn",
        morphotag_opts(false),
    )
    .await;
}

#[tokio::test]
async fn parity_morphotag_eng_bilingual() {
    run_parity_test(
        ReleasedCommand::Morphotag,
        InferTask::Morphosyntax,
        "eng_bilingual",
        "eng",
        morphotag_opts(false),
    )
    .await;
}
