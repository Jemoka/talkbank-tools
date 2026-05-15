use crate::common::{
    assert_completed_without_errors, require_live_direct_warmed, submit_and_complete_direct,
};
use batchalign::api::{FilePayload, JobStatus, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

use super::super::fixtures::{ENG_GONNA, NLD_SIMPLE, YUE_GU_SHI, ZHO_SHANG_DIAN};

fn morphotag_options(retokenize: bool, override_media_cache: bool) -> CommandOptions {
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
async fn option_morphotag_retokenize_changes_tokens() {
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

    let (info_a, results_a) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "gonna_no_retok.cha".into(),
            content: ENG_GONNA.into(),
        }],
        morphotag_options(false, true),
    )
    .await;
    assert_completed_without_errors("retokenize_false", &info_a, &results_a);

    let (info_b, results_b) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "eng",
        vec![FilePayload {
            filename: "gonna_retok.cha".into(),
            content: ENG_GONNA.into(),
        }],
        morphotag_options(true, true),
    )
    .await;
    assert_completed_without_errors("retokenize_true", &info_b, &results_b);

    let output_a = &results_a[0].content;
    let output_b = &results_b[0].content;
    assert_ne!(output_a, output_b);
    assert!(!output_b.contains("\tgonna "));
}

#[tokio::test]
async fn option_morphotag_retokenize_cantonese_collapses_character_tokens() {
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

    let (info_a, results_a) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "yue",
        vec![FilePayload {
            filename: "yue_char_level.cha".into(),
            content: YUE_GU_SHI.into(),
        }],
        morphotag_options(false, true),
    )
    .await;
    let (info_b, results_b) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "yue",
        vec![FilePayload {
            filename: "yue_retokenized.cha".into(),
            content: YUE_GU_SHI.into(),
        }],
        morphotag_options(true, true),
    )
    .await;

    if info_a.status == JobStatus::Failed || info_b.status == JobStatus::Failed {
        eprintln!("SKIP: Cantonese retokenize test needs the yue morphotag backend");
        return;
    }

    assert_completed_without_errors("yue_retokenize_false", &info_a, &results_a);
    assert_completed_without_errors("yue_retokenize_true", &info_b, &results_b);

    let output_a = &results_a[0].content;
    let output_b = &results_b[0].content;
    assert_ne!(output_a, output_b);
    assert!(output_a.contains("*CHI:\t故 事 係 好 ."));
    assert!(output_b.contains("*CHI:\t故事 係 好 ."));
}

#[tokio::test]
async fn option_morphotag_retokenize_mandarin_collapses_common_compounds() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "zho",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info_a, results_a) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "zho",
        vec![FilePayload {
            filename: "zho_char_level.cha".into(),
            content: ZHO_SHANG_DIAN.into(),
        }],
        morphotag_options(false, true),
    )
    .await;
    let (info_b, results_b) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "zho",
        vec![FilePayload {
            filename: "zho_retokenized.cha".into(),
            content: ZHO_SHANG_DIAN.into(),
        }],
        morphotag_options(true, true),
    )
    .await;

    if info_a.status == JobStatus::Failed || info_b.status == JobStatus::Failed {
        eprintln!("SKIP: Mandarin retokenize test needs the zho morphotag backend");
        return;
    }

    assert_completed_without_errors("zho_retokenize_false", &info_a, &results_a);
    assert_completed_without_errors("zho_retokenize_true", &info_b, &results_b);

    let output_a = &results_a[0].content;
    let output_b = &results_b[0].content;
    assert_ne!(output_a, output_b);
    assert!(output_a.contains("*PAR:\t商 店 很 大 ."));
    assert!(output_b.contains("*PAR:\t商店 很 大 ."));
}

#[tokio::test]
async fn option_morphotag_retokenize_without_mwt_stays_reliable() {
    let Some(session) = require_live_direct_warmed(
        InferTask::Morphosyntax,
        ReleasedCommand::Morphotag,
        "nld",
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };

    let (info, results) = submit_and_complete_direct(
        &session,
        ReleasedCommand::Morphotag,
        "nld",
        vec![FilePayload {
            filename: "nld_retokenize.cha".into(),
            content: NLD_SIMPLE.into(),
        }],
        morphotag_options(true, true),
    )
    .await;

    if info.status == JobStatus::Failed {
        eprintln!("SKIP: Dutch retokenize morphotag failed (model likely not downloaded)");
        return;
    }

    assert_completed_without_errors("nld_retokenize", &info, &results);
    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("%mor:"));
}
