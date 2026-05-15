use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, CompareOptions};
use batchalign::worker::InferTask;

use crate::ml_golden::golden::fixtures::{COMPARE_GOLD, COMPARE_MAIN};
use crate::ml_golden::golden::helpers::{
    assert_golden_snapshot, has_user_defined_tier, parse_output, require_direct_session_warmed,
    unique_test_dir,
};

#[tokio::test]
async fn golden_compare_eng() {
    let Some(session) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Compare,
        "eng",
        "Direct session does not support morphosyntax infer (required for compare)",
    )
    .await
    else {
        return;
    };

    let options = CommandOptions::Compare(CompareOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });
    let run_dir = unique_test_dir("compare_eng");

    let (info, results) = session
        .submit_files_job(
            ReleasedCommand::Compare,
            "eng",
            vec![
                FilePayload {
                    filename: format!("{run_dir}/compare_test.cha").into(),
                    content: COMPARE_MAIN.into(),
                },
                FilePayload {
                    filename: format!("{run_dir}/compare_test.gold.cha").into(),
                    content: COMPARE_GOLD.into(),
                },
            ],
            options,
        )
        .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none());

    let file = parse_output(&results[0].content, "compare_eng");
    assert!(has_user_defined_tier(&file, "xsrep"));
    assert!(!results[0].content.contains("%mor:"));
    assert!(!results[0].content.contains("%gra:"));
    assert_golden_snapshot!("compare_eng", &results[0].content);
}

#[tokio::test]
async fn golden_compare_uses_template_gold_fallback() {
    let Some(session) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Compare,
        "eng",
        "Direct session does not support morphosyntax infer (required for compare)",
    )
    .await
    else {
        return;
    };

    let options = CommandOptions::Compare(CompareOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });
    let run_dir = unique_test_dir("compare_template");

    let (info, results) = session
        .submit_files_job(
            ReleasedCommand::Compare,
            "eng",
            vec![
                FilePayload {
                    filename: format!("{run_dir}/compare_template.cha").into(),
                    content: COMPARE_MAIN.into(),
                },
                FilePayload {
                    filename: format!("{run_dir}/template.gold.cha").into(),
                    content: COMPARE_GOLD.into(),
                },
            ],
            options,
        )
        .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none());

    let file = parse_output(&results[0].content, "compare_template_gold");
    assert!(has_user_defined_tier(&file, "xsrep"));
    assert!(!results[0].content.contains("%mor:"));
    assert!(!results[0].content.contains("%gra:"));
}

#[tokio::test]
async fn golden_compare_sequential_jobs_stay_isolated() {
    let Some(session) = require_direct_session_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Compare,
        "eng",
        "Direct session does not support morphosyntax infer (required for compare)",
    )
    .await
    else {
        return;
    };

    let options = CommandOptions::Compare(CompareOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        merge_abbrev: false.into(),
    });

    let first_dir = unique_test_dir("compare_seq_first");
    let (first_info, first_results) = session
        .submit_files_job(
            ReleasedCommand::Compare,
            "eng",
            vec![
                FilePayload {
                    filename: format!("{first_dir}/compare_test.cha").into(),
                    content: COMPARE_MAIN.into(),
                },
                FilePayload {
                    filename: format!("{first_dir}/compare_test.gold.cha").into(),
                    content: COMPARE_GOLD.into(),
                },
            ],
            options.clone(),
        )
        .await;
    assert_eq!(first_info.status, JobStatus::Completed);
    assert_eq!(first_results.len(), 1);
    assert!(first_results[0].error.is_none(), "{first_results:#?}");

    let second_dir = unique_test_dir("compare_seq_second");
    let (second_info, second_results) = session
        .submit_files_job(
            ReleasedCommand::Compare,
            "eng",
            vec![
                FilePayload {
                    filename: format!("{second_dir}/compare_template.cha").into(),
                    content: COMPARE_MAIN.into(),
                },
                FilePayload {
                    filename: format!("{second_dir}/template.gold.cha").into(),
                    content: COMPARE_GOLD.into(),
                },
            ],
            options,
        )
        .await;
    assert_eq!(second_info.status, JobStatus::Completed);
    assert_eq!(second_results.len(), 1);
    assert!(second_results[0].error.is_none(), "{second_results:#?}");
}

#[test]
fn golden_compare_warmed_session_survives_tokio_runtime_restart() {
    let build_runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread runtime")
    };

    let run_once = |label: &str, filename: &str| {
        build_runtime().block_on(async move {
            let Some(session) = require_direct_session_warmed(
                InferTask::Morphosyntax,
                ReleasedCommand::Compare,
                "eng",
                "Direct session does not support morphosyntax infer (required for compare)",
            )
            .await
            else {
                return;
            };

            let options = CommandOptions::Compare(CompareOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    ..CommonOptions::default()
                },
                merge_abbrev: false.into(),
            });
            let run_dir = unique_test_dir(label);
            let (info, results) = session
                .submit_files_job(
                    ReleasedCommand::Compare,
                    "eng",
                    vec![
                        FilePayload {
                            filename: format!("{run_dir}/{filename}.cha").into(),
                            content: COMPARE_MAIN.into(),
                        },
                        FilePayload {
                            filename: format!("{run_dir}/template.gold.cha").into(),
                            content: COMPARE_GOLD.into(),
                        },
                    ],
                    options,
                )
                .await;
            assert_eq!(info.status, JobStatus::Completed);
            assert_eq!(results.len(), 1);
            assert!(results[0].error.is_none(), "{results:#?}");
        });
    };

    run_once("compare_runtime_first", "compare_first");
    run_once("compare_runtime_second", "compare_second");
}
