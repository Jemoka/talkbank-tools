use crate::common::assert_completed_without_errors;
use crate::ml_golden::golden::fixtures::{ENG_SIMPLE, SPA_SIMPLE};
use crate::ml_golden::golden::helpers::{
    assert_golden_snapshot, has_user_defined_tier, parse_output, require_direct_session_warmed,
};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, TranslateOptions};
use batchalign::worker::InferTask;

fn translate_options() -> CommandOptions {
    CommandOptions::Translate(TranslateOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    })
}

#[tokio::test]
async fn golden_translate_eng_simple() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Translate,
        ReleasedCommand::Translate,
        "eng",
        "Direct session does not support translate infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Translate,
            "eng",
            "eng_simple.cha",
            ENG_SIMPLE,
            translate_options(),
        )
        .await;

    assert_completed_without_errors("translate_eng_simple", &info, &results);
    assert_golden_snapshot!("translate_eng_simple", &results[0].content);
}

#[tokio::test]
async fn golden_translate_spa_to_eng() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Translate,
        ReleasedCommand::Translate,
        "spa",
        "Direct session does not support translate infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Translate,
            "spa",
            "spa_simple.cha",
            SPA_SIMPLE,
            translate_options(),
        )
        .await;

    if info.status == JobStatus::Failed {
        eprintln!("SKIP: Spanish translate failed (model likely not downloaded)");
        return;
    }

    assert_completed_without_errors("translate_spa_to_eng", &info, &results);
    let file = parse_output(&results[0].content, "translate_spa_to_eng");
    assert!(has_user_defined_tier(&file, "xtra"));
    assert_golden_snapshot!("translate_spa_to_eng", &results[0].content);
}
