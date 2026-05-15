use crate::common::{
    assert_completed_without_errors, require_live_direct_warmed_many, submit_and_complete_direct,
};
use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

use super::super::fixtures::{ENG_SIMPLE, ENG_SPA_PRECODE};
use super::super::helpers::count_mor_lines;

fn morphotag_options(skipmultilang: bool) -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        skipmultilang,

        ..Default::default()
    })
}

#[tokio::test]
async fn option_morphotag_skipmultilang() {
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

    let (info_a, results_a) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "bilingual_precode_all.cha".into(),
            content: ENG_SPA_PRECODE.into(),
        }],
        morphotag_options(false),
    )
    .await;
    let (info_b, results_b) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "bilingual_precode_skip.cha".into(),
            content: ENG_SPA_PRECODE.into(),
        }],
        morphotag_options(true),
    )
    .await;

    if info_a.status == JobStatus::Failed || info_b.status == JobStatus::Failed {
        eprintln!("SKIP: skipmultilang differential test needs both English and Spanish models");
        return;
    }

    assert_completed_without_errors("skipmultilang_false", &info_a, &results_a);
    assert_completed_without_errors("skipmultilang_true", &info_b, &results_b);

    assert_eq!(count_mor_lines(&results_a[0].content), 2);
    assert_eq!(count_mor_lines(&results_b[0].content), 1);
    assert!(results_b[0].content.contains("*CHI:\t[- spa] hola mundo ."));
    assert_ne!(results_a[0].content, results_b[0].content);
}

#[tokio::test]
async fn option_morphotag_skipmultilang_only_skips_multilingual_neighbors() {
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

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![
            FilePayload {
                filename: "mono_neighbor.cha".into(),
                content: ENG_SIMPLE.into(),
            },
            FilePayload {
                filename: "bilingual_skipped.cha".into(),
                content: ENG_SPA_PRECODE.into(),
            },
        ],
        morphotag_options(true),
    )
    .await;

    assert_completed_without_errors("skipmultilang_mixed_batch", &info, &results);
    assert_eq!(results.len(), 2);

    let mono = results
        .iter()
        .find(|result| result.filename == "mono_neighbor.cha")
        .expect("monolingual file result");
    let bilingual = results
        .iter()
        .find(|result| result.filename == "bilingual_skipped.cha")
        .expect("bilingual file result");

    assert!(count_mor_lines(&mono.content) == 2);
    assert!(count_mor_lines(&bilingual.content) == 1);
    assert!(bilingual.content.contains("*CHI:\t[- spa] hola mundo ."));
}
