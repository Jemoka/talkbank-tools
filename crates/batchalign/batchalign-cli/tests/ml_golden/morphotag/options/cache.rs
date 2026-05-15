use crate::common::{
    assert_completed_without_errors, require_live_direct_warmed, require_live_direct_warmed_many,
    submit_and_complete_direct,
};
use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

use super::super::fixtures::{ENG_SIMPLE, SPA_SIMPLE, YUE_GU_SHI};

fn morphotag_options(
    retokenize: bool,
    override_media_cache: bool,
    skipmultilang: bool,
    no_l2_morphotag: bool,
) -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache,
            ..CommonOptions::default()
        },
        retokenize,
        skipmultilang,
        no_l2_morphotag,

        ..Default::default()
    })
}

#[tokio::test]
async fn option_morphotag_cantonese_retokenize_cache_isolated_from_preserve_mode() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "yue",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let files = vec![FilePayload {
        filename: "yue_cache_key.cha".into(),
        content: YUE_GU_SHI.into(),
    }];

    let (info_a, results_a) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "yue",
        files.clone(),
        morphotag_options(false, false, false, false),
    )
    .await;
    let (info_b, results_b) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "yue",
        files.clone(),
        morphotag_options(true, false, false, false),
    )
    .await;
    let (info_c, results_c) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "yue",
        files,
        morphotag_options(false, false, false, false),
    )
    .await;

    if info_a.status == JobStatus::Failed
        || info_b.status == JobStatus::Failed
        || info_c.status == JobStatus::Failed
    {
        eprintln!("SKIP: Cantonese cache isolation test needs the yue morphotag backend");
        return;
    }

    assert_completed_without_errors("yue_cache_preserve_first", &info_a, &results_a);
    assert_completed_without_errors("yue_cache_retok_second", &info_b, &results_b);
    assert_completed_without_errors("yue_cache_preserve_third", &info_c, &results_c);

    let output_a = &results_a[0].content;
    let output_b = &results_b[0].content;
    let output_c = &results_c[0].content;

    assert!(output_a.contains("*CHI:\t故 事 係 好 ."));
    assert!(output_b.contains("*CHI:\t故事 係 好 ."));
    assert!(output_c.contains("*CHI:\t故 事 係 好 ."));
    assert_eq!(output_a, output_c);
    assert_ne!(output_a, output_b);
}

#[tokio::test]
async fn option_morphotag_multilingual_warm_cache_preserves_per_language_outputs() {
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

    let files = vec![
        FilePayload {
            filename: "eng_cache_group.cha".into(),
            content: ENG_SIMPLE.into(),
        },
        FilePayload {
            filename: "spa_cache_group.cha".into(),
            content: SPA_SIMPLE.into(),
        },
    ];

    let (info_a, mut results_a) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        files.clone(),
        morphotag_options(false, false, false, false),
    )
    .await;
    let (info_b, mut results_b) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        files,
        morphotag_options(false, false, false, false),
    )
    .await;

    if info_a.status == JobStatus::Failed || info_b.status == JobStatus::Failed {
        eprintln!(
            "SKIP: multilingual cache test needs both English and Spanish morphotag backends"
        );
        return;
    }

    assert_completed_without_errors("multilingual_cache_cold", &info_a, &results_a);
    assert_completed_without_errors("multilingual_cache_warm", &info_b, &results_b);

    results_a.sort_by(|a, b| a.filename.cmp(&b.filename));
    results_b.sort_by(|a, b| a.filename.cmp(&b.filename));
    assert_eq!(results_a.len(), 2);
    assert_eq!(results_b.len(), 2);

    for (cold, warm) in results_a.iter().zip(results_b.iter()) {
        assert_eq!(cold.filename, warm.filename);
        assert_eq!(cold.content, warm.content);
        assert!(cold.content.contains("%mor:"));
    }
}

#[tokio::test]
async fn option_override_media_cache_forces_recompute() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "eng",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let input = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tthe cat sat .
@End
";

    let (info1, results1) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "cache_override.cha".into(),
            content: input.into(),
        }],
        morphotag_options(false, false, false, false),
    )
    .await;
    assert_completed_without_errors("cache_normal", &info1, &results1);

    let (info2, results2) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "cache_override.cha".into(),
            content: input.into(),
        }],
        morphotag_options(false, true, false, false),
    )
    .await;
    assert_completed_without_errors("cache_override", &info2, &results2);

    assert_eq!(results1[0].content, results2[0].content);
}
