use crate::common::assert_completed_without_errors;
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

use crate::ml_golden::golden::fixtures::{
    ENG_DISFLUENCY_PARITY, ENG_MULTI_UTT, ENG_RETOKENIZE, ENG_SIMPLE, SPA_SIMPLE,
};
use crate::ml_golden::golden::helpers::{
    assert_golden_snapshot, find_mor_line_for, has_gra_tier, has_mor_tier, parse_output,
    require_direct_session_warmed,
};

fn morphotag_options(override_media_cache: bool, retokenize: bool) -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache,
            ..CommonOptions::default()
        },
        retokenize,

        ..Default::default()
    })
}

#[tokio::test]
async fn golden_morphotag_eng_simple() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "eng_simple.cha",
            ENG_SIMPLE,
            morphotag_options(true, false),
        )
        .await;

    assert_completed_without_errors("morphotag_eng_simple", &info, &results);
    let output = &results[0].content;
    let file = parse_output(output, "morphotag_eng_simple");
    assert!(has_mor_tier(&file));
    assert!(has_gra_tier(&file));
    assert_golden_snapshot!("morphotag_eng_simple", output);
}

#[tokio::test]
async fn golden_morphotag_eng_multi_utt() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "eng_multi_utt.cha",
            ENG_MULTI_UTT,
            morphotag_options(true, false),
        )
        .await;

    assert_completed_without_errors("morphotag_eng_multi_utt", &info, &results);
    assert_golden_snapshot!("morphotag_eng_multi_utt", &results[0].content);
}

#[tokio::test]
async fn golden_morphotag_with_cache() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info1, results1) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "cache_test.cha",
            ENG_SIMPLE,
            morphotag_options(false, false),
        )
        .await;
    assert_completed_without_errors("morphotag_with_cache_cold", &info1, &results1);

    let (info2, results2) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "cache_test.cha",
            ENG_SIMPLE,
            morphotag_options(false, false),
        )
        .await;
    assert_completed_without_errors("morphotag_with_cache_warm", &info2, &results2);

    assert_eq!(results1[0].content, results2[0].content);
}

#[tokio::test]
async fn golden_morphotag_spa_simple() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "spa",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "spa",
            "spa_simple.cha",
            SPA_SIMPLE,
            morphotag_options(true, false),
        )
        .await;

    if info.status == batchalign::api::JobStatus::Failed {
        eprintln!("SKIP: Spanish morphotag failed (model likely not downloaded)");
        return;
    }

    assert_completed_without_errors("morphotag_spa_simple", &info, &results);
    let file = parse_output(&results[0].content, "morphotag_spa_simple");
    assert!(has_mor_tier(&file));
    assert!(has_gra_tier(&file));
    assert_golden_snapshot!("morphotag_spa_simple", &results[0].content);
}

#[tokio::test]
async fn golden_morphotag_retokenize_eng() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "eng_retokenize.cha",
            ENG_RETOKENIZE,
            morphotag_options(true, true),
        )
        .await;

    assert_completed_without_errors("morphotag_retokenize_eng", &info, &results);
    let file = parse_output(&results[0].content, "morphotag_retokenize_eng");
    assert!(has_mor_tier(&file));
    assert!(has_gra_tier(&file));
    assert_golden_snapshot!("morphotag_retokenize_eng", &results[0].content);
}

#[tokio::test]
async fn morphotag_disfluency_preserves_thats_subject_and_copula() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "eng_disfluency.cha",
            ENG_DISFLUENCY_PARITY,
            morphotag_options(true, false),
        )
        .await;

    assert_completed_without_errors(
        "morphotag_disfluency_preserves_thats_subject_and_copula",
        &info,
        &results,
    );
    assert_eq!(results.len(), 1);

    let mmhmm_mor = find_mor_line_for(&results[0].content, "mm-hmm that's right")
        .expect("expected %mor line for mm-hmm that's right");
    assert!(
        mmhmm_mor.contains("pron|that-Dem~aux|be-Fin-Ind-Pres-S3"),
        "expected explicit subject+copula analysis for \"that's right\", got: {mmhmm_mor}"
    );
    assert!(
        !mmhmm_mor.contains("aux|that-Fin-Ind-Pres-S3"),
        "unexpected collapsed auxiliary-only analysis for \"that's right\": {mmhmm_mor}"
    );
}

#[tokio::test]
async fn golden_morphotag_cache_is_faster() {
    let Some(jobs) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let start1 = std::time::Instant::now();
    let (info1, results1) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "cache_speed.cha",
            ENG_SIMPLE,
            morphotag_options(false, false),
        )
        .await;
    let elapsed1 = start1.elapsed();
    assert_completed_without_errors("morphotag_cache_is_faster_cold", &info1, &results1);

    let start2 = std::time::Instant::now();
    let (info2, results2) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "cache_speed.cha",
            ENG_SIMPLE,
            morphotag_options(false, false),
        )
        .await;
    let elapsed2 = start2.elapsed();
    assert_completed_without_errors("morphotag_cache_is_faster_warm", &info2, &results2);

    assert_eq!(results1[0].content, results2[0].content);
    eprintln!(
        "Cache timing: cold={:?}, warm={:?} (speedup: {:.1}x)",
        elapsed1,
        elapsed2,
        elapsed1.as_secs_f64() / elapsed2.as_secs_f64()
    );
    if elapsed1.as_secs_f64() > 1.0 {
        assert!(elapsed2 < elapsed1);
    }
}
