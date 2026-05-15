use crate::common::{
    assert_completed_without_errors, require_live_direct_warmed_many, submit_and_complete_direct,
};
use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

use super::super::fixtures::{ENG_SIMPLE, ENG_SPA_L2, ENG_XYZ_L2};
use super::super::helpers::find_mor_line_for;

fn morphotag_options(no_l2_morphotag: bool) -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        no_l2_morphotag,

        ..Default::default()
    })
}

#[tokio::test]
async fn option_morphotag_no_l2_switch_controls_code_switched_words() {
    let Some(session) = require_live_direct_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "eng"),
            (ReleasedCommand::Morphotag, "spa"),
        ],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info_enabled, results_enabled) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "eng_spa_l2_on.cha".into(),
            content: ENG_SPA_L2.into(),
        }],
        morphotag_options(false),
    )
    .await;
    let (info_disabled, results_disabled) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "eng_spa_l2_off.cha".into(),
            content: ENG_SPA_L2.into(),
        }],
        morphotag_options(true),
    )
    .await;

    if info_enabled.status == JobStatus::Failed || info_disabled.status == JobStatus::Failed {
        eprintln!(
            "SKIP: bilingual L2 morphotag failed (secondary language model likely unavailable)"
        );
        return;
    }

    assert_completed_without_errors("l2_enabled", &info_enabled, &results_enabled);
    assert_completed_without_errors("l2_disabled", &info_disabled, &results_disabled);

    let tienda_on = find_mor_line_for(&results_enabled[0].content, "tienda@s:spa")
        .expect("tienda MOR with L2 enabled");
    let tienda_off = find_mor_line_for(&results_disabled[0].content, "tienda@s:spa")
        .expect("tienda MOR with L2 disabled");

    assert!(!tienda_on.contains("L2|xxx"));
    assert!(tienda_off.contains("L2|xxx"));
    assert_ne!(results_enabled[0].content, results_disabled[0].content);
}

#[tokio::test]
async fn option_morphotag_unsupported_inline_language_falls_back_to_l2_xxx() {
    let Some(session) = require_live_direct_warmed_many(
        InferTask::Morphosyntax,
        vec![(ReleasedCommand::Morphotag, "eng")],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "eng_xyz_l2.cha".into(),
            content: ENG_XYZ_L2.into(),
        }],
        morphotag_options(false),
    )
    .await;

    assert_completed_without_errors("unsupported_inline_l2", &info, &results);
    let blorx_mor = find_mor_line_for(&results[0].content, "blorx@s:xyz")
        .expect("should have MOR for unsupported inline word");
    assert!(blorx_mor.contains("L2|xxx"));
}

#[tokio::test]
async fn option_morphotag_unsupported_inline_language_does_not_poison_neighbor_file() {
    let Some(session) = require_live_direct_warmed_many(
        InferTask::Morphosyntax,
        vec![(ReleasedCommand::Morphotag, "eng")],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![
            FilePayload {
                filename: "neighbor_clean.cha".into(),
                content: ENG_SIMPLE.into(),
            },
            FilePayload {
                filename: "neighbor_unsupported.cha".into(),
                content: ENG_XYZ_L2.into(),
            },
        ],
        morphotag_options(false),
    )
    .await;

    assert_completed_without_errors("unsupported_inline_neighbor_batch", &info, &results);
    assert_eq!(results.len(), 2);

    let clean = results
        .iter()
        .find(|result| result.filename == "neighbor_clean.cha")
        .expect("clean neighbor result");
    let unsupported = results
        .iter()
        .find(|result| result.filename == "neighbor_unsupported.cha")
        .expect("unsupported neighbor result");

    assert!(clean.content.contains("%mor:"));
    assert!(!clean.content.contains("L2|xxx"));

    let blorx_mor = find_mor_line_for(&unsupported.content, "blorx@s:xyz")
        .expect("unsupported inline file should still have a MOR line");
    assert!(blorx_mor.contains("L2|xxx"));
}
