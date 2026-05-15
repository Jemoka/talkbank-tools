//! Direct-path morphotag behavior tests.

use super::fixtures::{
    DIRECT_ENG_AFTER_INCREMENTAL, DIRECT_ENG_BEFORE_INCREMENTAL, DIRECT_ENG_FILE_A,
    DIRECT_ENG_FILE_B, DIRECT_SPEAKER_FATHER, DIRECT_SPEAKER_MOTHER,
};
use super::helpers::{count_ast_mor_tiers, has_mor_tier, parse_output, strip_ba3_comments};
use crate::common::{
    LiveDirectJobClient, assert_completed_without_errors, require_live_direct_warmed,
};
use batchalign::api::{FilePayload, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

fn default_morphotag_options() -> CommandOptions {
    CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },

        ..Default::default()
    })
}

/// Morphotag with multiple files verifies batching and independent output
/// materialization on the direct path.
#[tokio::test]
async fn direct_morphotag_multi_file_batching() {
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
    let jobs = LiveDirectJobClient::new(&session);

    let files = vec![
        FilePayload {
            filename: "file_a.cha".into(),
            content: DIRECT_ENG_FILE_A.into(),
        },
        FilePayload {
            filename: "file_b.cha".into(),
            content: DIRECT_ENG_FILE_B.into(),
        },
    ];

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            files,
            default_morphotag_options(),
        )
        .await;

    assert_completed_without_errors("multi_file_morphotag", &info, &results);
    assert_eq!(results.len(), 2, "Should produce 2 output files");

    let file_a = parse_output(&results[0].content, "file_a");
    let file_b = parse_output(&results[1].content, "file_b");

    assert_eq!(
        count_ast_mor_tiers(&file_a),
        3,
        "file_a: all 3 utterances should have %mor"
    );
    assert_eq!(
        count_ast_mor_tiers(&file_b),
        3,
        "file_b: all 3 utterances should have %mor"
    );
}

/// Morphotag with two independent English files verifies grouped dispatch does
/// not collapse file-local outputs.
#[tokio::test]
async fn direct_morphotag_multi_speaker_batching() {
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
    let jobs = LiveDirectJobClient::new(&session);

    let files = vec![
        FilePayload {
            filename: "speaker_a.cha".into(),
            content: DIRECT_SPEAKER_MOTHER.into(),
        },
        FilePayload {
            filename: "speaker_b.cha".into(),
            content: DIRECT_SPEAKER_FATHER.into(),
        },
    ];

    let (info, results) = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            "eng",
            files,
            default_morphotag_options(),
        )
        .await;

    assert_completed_without_errors("multi_speaker_morphotag", &info, &results);
    assert_eq!(results.len(), 2, "Should produce 2 output files");

    let file_a = parse_output(&results[0].content, "speaker_a");
    let file_b = parse_output(&results[1].content, "speaker_b");
    assert!(has_mor_tier(&file_a), "speaker_a should have %mor tier");
    assert!(has_mor_tier(&file_b), "speaker_b should have %mor tier");
    assert_eq!(count_ast_mor_tiers(&file_a), 3, "speaker_a: 3 utterances");
    assert_eq!(count_ast_mor_tiers(&file_b), 3, "speaker_b: 3 utterances");
}

/// Morphotag incremental reruns with `--before` should preserve full-run
/// output semantics for the edited file.
#[tokio::test]
async fn direct_morphotag_before_matches_full_rerun_output() {
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
    let jobs = LiveDirectJobClient::new(&session);

    let input_dir = jobs.state_dir().join("incremental_inputs");
    let before_seed_dir = jobs.state_dir().join("incremental_before_seed");
    let before_dir = jobs.state_dir().join("incremental_before");
    let out_full_dir = jobs.state_dir().join("incremental_full_out");
    let out_before_dir = jobs.state_dir().join("incremental_before_out");
    std::fs::create_dir_all(&input_dir).expect("mkdir incremental inputs");
    std::fs::create_dir_all(&before_seed_dir).expect("mkdir incremental before seed");
    std::fs::create_dir_all(&before_dir).expect("mkdir incremental before");
    std::fs::create_dir_all(&out_full_dir).expect("mkdir incremental full out");
    std::fs::create_dir_all(&out_before_dir).expect("mkdir incremental before out");

    let source_path = input_dir.join("edited.cha");
    let before_seed_path = before_seed_dir.join("edited.cha");
    let before_seed_output_path = before_dir.join("edited.cha");
    let before_path = before_dir.join("edited.cha");
    let full_output_path = out_full_dir.join("edited.cha");
    let incremental_output_path = out_before_dir.join("edited.cha");
    std::fs::write(&source_path, DIRECT_ENG_AFTER_INCREMENTAL).expect("write edited input");
    std::fs::write(&before_seed_path, DIRECT_ENG_BEFORE_INCREMENTAL)
        .expect("write before seed input");

    let options = default_morphotag_options();

    let (full_info, full_outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Morphotag,
            "eng",
            vec![source_path.to_string_lossy().into()],
            vec![full_output_path.to_string_lossy().into()],
            options.clone(),
        )
        .await;
    assert_completed_without_errors("morphotag_full_rerun", &full_info, &[]);

    let (before_seed_info, _before_seed_outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Morphotag,
            "eng",
            vec![before_seed_path.to_string_lossy().into()],
            vec![before_seed_output_path.to_string_lossy().into()],
            options.clone(),
        )
        .await;
    assert_completed_without_errors("morphotag_before_seed", &before_seed_info, &[]);

    let (incremental_info, incremental_outputs) = jobs
        .submit_paths_job_with_before(
            ReleasedCommand::Morphotag,
            "eng",
            vec![source_path.to_string_lossy().into()],
            vec![incremental_output_path.to_string_lossy().into()],
            vec![before_path.to_string_lossy().into()],
            options,
        )
        .await;
    assert_completed_without_errors("morphotag_incremental_before", &incremental_info, &[]);

    assert_eq!(full_outputs.len(), 1);
    assert_eq!(incremental_outputs.len(), 1);

    let full_file = parse_output(&full_outputs[0], "morphotag_full_rerun");
    let incremental_file = parse_output(&incremental_outputs[0], "morphotag_incremental_before");
    assert_eq!(
        count_ast_mor_tiers(&full_file),
        2,
        "full rerun should tag both utterances"
    );
    assert_eq!(
        count_ast_mor_tiers(&incremental_file),
        2,
        "incremental rerun should tag both utterances"
    );

    assert_eq!(
        strip_ba3_comments(&full_outputs[0]),
        strip_ba3_comments(&incremental_outputs[0]),
        "incremental morphotag output should match full rerun semantics"
    );
}
