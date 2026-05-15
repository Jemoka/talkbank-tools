use crate::common::assert_completed_without_errors;
use batchalign::api::ReleasedCommand;
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

use crate::ml_golden::golden::fixtures::{
    CAT_SPA_L2, DAN_ENG_L2, DEU_ENG_CONTRACTIONS, DEU_ENG_L2, DEU_ENG_PHRASAL, ENG_SPA_L2,
    FRA_NLD_L2,
};
use crate::ml_golden::golden::helpers::{
    assert_golden_snapshot, find_mor_line_for, require_direct_session_warmed_many,
};

fn l2_enabled_options() -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },

        ..Default::default()
    })
}

fn l2_disabled_options() -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        no_l2_morphotag: true,

        ..Default::default()
    })
}

#[tokio::test]
async fn golden_l2_morphotag_eng_spa() {
    let Some(jobs) = require_direct_session_warmed_many(
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
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "eng_spa_l2.cha",
            ENG_SPA_L2,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_eng_spa", &info, &results);
    let output = &results[0].content;
    assert!(!output.contains("L2|xxx"));
    assert!(
        find_mor_line_for(output, "tienda@s:spa")
            .unwrap()
            .contains("noun|tienda")
    );
    assert!(
        find_mor_line_for(output, "muy@s:spa")
            .unwrap()
            .contains("adv|")
    );
    assert!(
        find_mor_line_for(output, "niños@s:spa")
            .unwrap()
            .contains("niño")
    );
    assert_golden_snapshot!("l2_morphotag_eng_spa", output);
}

#[tokio::test]
async fn golden_l2_morphotag_deu_eng() {
    let Some(jobs) = require_direct_session_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "deu"),
            (ReleasedCommand::Morphotag, "eng"),
        ],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "deu",
            "deu_eng_l2.cha",
            DEU_ENG_L2,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_deu_eng", &info, &results);
    let output = &results[0].content;
    assert!(!output.contains("L2|xxx"));
    assert!(
        find_mor_line_for(output, "film@s")
            .unwrap()
            .contains("noun|film")
    );
    assert!(
        find_mor_line_for(output, "drug@s")
            .unwrap()
            .contains("noun|drug")
    );
    assert_golden_snapshot!("l2_morphotag_deu_eng", output);
}

#[tokio::test]
async fn golden_l2_morphotag_eng_contractions() {
    let Some(jobs) = require_direct_session_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "deu"),
            (ReleasedCommand::Morphotag, "eng"),
        ],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "deu",
            "deu_eng_contractions.cha",
            DEU_ENG_CONTRACTIONS,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_eng_contractions", &info, &results);
    let output = &results[0].content;
    let its_mor = find_mor_line_for(output, "it's@s:eng").unwrap();
    assert!(its_mor.contains('~'));
    assert!(!its_mor.contains("L2|xxx"));
    let dont_mor = find_mor_line_for(output, "don't@s:eng").unwrap();
    assert!(dont_mor.contains('~'));
    let working_mor = find_mor_line_for(output, "working@s:eng").unwrap();
    assert!(!working_mor.contains("L2|xxx"));
    assert_golden_snapshot!("l2_morphotag_eng_contractions", output);
}

#[tokio::test]
async fn golden_l2_morphotag_phrasal_verbs() {
    let Some(jobs) = require_direct_session_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "deu"),
            (ReleasedCommand::Morphotag, "eng"),
        ],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "deu",
            "deu_eng_phrasal.cha",
            DEU_ENG_PHRASAL,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_phrasal_verbs", &info, &results);
    let output = &results[0].content;
    assert!(!output.contains("L2|xxx"));
    let wake_mor = find_mor_line_for(output, "wake@s up@s").unwrap();
    assert!(wake_mor.contains("verb|wake"));
    assert!(wake_mor.contains("part|up"));
    let give_mor = find_mor_line_for(output, "give@s up@s").unwrap();
    assert!(give_mor.contains("verb|give"));
    assert!(give_mor.contains("part|up"));
    let pick_mor = find_mor_line_for(output, "pick@s up@s").unwrap();
    assert!(pick_mor.contains("verb|pick"));
    assert!(pick_mor.contains("part|up"));
    let time_mor = find_mor_line_for(output, "time@s out@s").unwrap();
    assert!(time_mor.contains("noun|time"));
    assert!(time_mor.contains("adp|out"));
    assert_golden_snapshot!("l2_morphotag_phrasal_verbs", output);
}

#[tokio::test]
async fn golden_l2_morphotag_off_produces_l2_xxx() {
    let Some(jobs) = require_direct_session_warmed_many(
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
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            "eng_spa_l2_off.cha",
            ENG_SPA_L2,
            l2_disabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_off", &info, &results);
    let output = &results[0].content;
    assert!(output.contains("L2|xxx"));
    assert!(
        find_mor_line_for(output, "tienda@s:spa")
            .unwrap()
            .contains("L2|xxx")
    );
}

#[tokio::test]
async fn golden_l2_morphotag_cat_spa() {
    let Some(jobs) = require_direct_session_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "cat"),
            (ReleasedCommand::Morphotag, "spa"),
        ],
        "Direct session does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "cat",
            "cat_spa_l2.cha",
            CAT_SPA_L2,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_cat_spa", &info, &results);
    let output = &results[0].content;
    assert!(!output.contains("L2|xxx"));
    assert!(
        !find_mor_line_for(output, "cole@s")
            .unwrap()
            .contains("L2|xxx")
    );
    assert!(
        !find_mor_line_for(output, "bonita@s")
            .unwrap()
            .contains("L2|xxx")
    );
    assert_golden_snapshot!("l2_morphotag_cat_spa", output);
}

#[tokio::test]
async fn golden_l2_morphotag_dan_eng() {
    let Some(jobs) = require_direct_session_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "dan"),
            (ReleasedCommand::Morphotag, "eng"),
        ],
        "Direct session does not support morphotag infer",
    )
    .await
    else {
        return;
    };
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "dan",
            "dan_eng_l2.cha",
            DAN_ENG_L2,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_dan_eng", &info, &results);
    let output = &results[0].content;
    assert!(!output.contains("L2|xxx"));
    assert!(
        !find_mor_line_for(output, "computer@s")
            .unwrap()
            .contains("L2|xxx")
    );
    assert!(
        !find_mor_line_for(output, "happy@s")
            .unwrap()
            .contains("L2|xxx")
    );
    assert_golden_snapshot!("l2_morphotag_dan_eng", output);
}

#[tokio::test]
async fn golden_l2_morphotag_fra_nld() {
    let Some(jobs) = require_direct_session_warmed_many(
        InferTask::Morphosyntax,
        vec![
            (ReleasedCommand::Morphotag, "fra"),
            (ReleasedCommand::Morphotag, "nld"),
        ],
        "Direct session does not support morphotag infer",
    )
    .await
    else {
        return;
    };
    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "fra",
            "fra_nld_l2.cha",
            FRA_NLD_L2,
            l2_enabled_options(),
        )
        .await;
    assert_completed_without_errors("l2_morphotag_fra_nld", &info, &results);
    let output = &results[0].content;
    assert!(!output.contains("L2|xxx"));
    assert!(
        !find_mor_line_for(output, "opa@s")
            .unwrap()
            .contains("L2|xxx")
    );
    assert!(
        !find_mor_line_for(output, "ja@s:nld")
            .unwrap()
            .contains("L2|xxx")
    );
    assert_golden_snapshot!("l2_morphotag_fra_nld", output);
}
