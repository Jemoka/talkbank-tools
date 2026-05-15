use crate::ml_golden::golden::fixtures::{ENG_COREF, SPA_SIMPLE};
use crate::ml_golden::golden::helpers::{
    assert_golden_snapshot, has_user_defined_tier, parse_output, require_direct_session_warmed,
};
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, CorefOptions};
use batchalign::worker::InferTask;

#[tokio::test]
async fn golden_coref_eng() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Coref,
        ReleasedCommand::Coref,
        "eng",
        "Direct session does not support coref infer",
    )
    .await
    else {
        return;
    };

    let options = CommandOptions::Coref(CorefOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Coref,
            "eng",
            "eng_coref.cha",
            ENG_COREF,
            options,
        )
        .await;

    assert_eq!(info.status, batchalign::api::JobStatus::Completed);
    let file = parse_output(&results[0].content, "coref_eng");
    if has_user_defined_tier(&file, "xcoref") {
        eprintln!("Coref model detected chains — snapshotting with %xcoref");
    } else {
        eprintln!("Coref model found no chains (valid for short input)");
    }
    assert_golden_snapshot!("coref_eng", &results[0].content);
}

#[tokio::test]
async fn golden_coref_spa_passthrough() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Coref,
        ReleasedCommand::Coref,
        "eng",
        "Direct session does not support coref infer",
    )
    .await
    else {
        return;
    };

    let options = CommandOptions::Coref(CorefOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Coref,
            "eng",
            "spa_simple.cha",
            SPA_SIMPLE,
            options,
        )
        .await;

    assert_eq!(info.status, batchalign::api::JobStatus::Completed);
    let file = parse_output(&results[0].content, "coref_spa_passthrough");
    assert!(
        !has_user_defined_tier(&file, "xcoref"),
        "non-English coref input should pass through without %xcoref"
    );
    assert_golden_snapshot!("coref_spa_passthrough", &results[0].content);
}
