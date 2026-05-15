use super::super::fixtures::{ENG_SIMPLE_SERVER, ENG_SPA_L2, ENG_SPA_PRECODE, ENG_XYZ_L2};
use super::super::helpers::{count_mor_lines, find_mor_line_for};
use crate::common::{LiveServerJobClient, require_live_server};
use batchalign::api::{FilePayload, JobStatus, LanguageCode3, LanguageSpec, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

/// The server path must preserve the `no_l2_morphotag` option boundary so
/// bilingual `@s:` words either get real secondary-language morphology or stay
/// on the `L2|xxx` fallback path depending on the flag.
#[tokio::test]
async fn morphotag_server_no_l2_switch_controls_code_switched_words() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let enabled = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            vec![FilePayload {
                filename: "server_l2_on.cha".into(),
                content: ENG_SPA_L2.into(),
            }],
            CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    batch_window: 0,
                    ..CommonOptions::default()
                },

                ..Default::default()
            }),
        )
        .await;
    let enabled_info = jobs.poll_done(&enabled.job_id).await;
    let enabled_results = jobs.job_results(&enabled.job_id).await;

    let disabled = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            vec![FilePayload {
                filename: "server_l2_off.cha".into(),
                content: ENG_SPA_L2.into(),
            }],
            CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    batch_window: 0,
                    ..CommonOptions::default()
                },
                no_l2_morphotag: true,

                ..Default::default()
            }),
        )
        .await;
    let disabled_info = jobs.poll_done(&disabled.job_id).await;
    let disabled_results = jobs.job_results(&disabled.job_id).await;

    if enabled_info.status == JobStatus::Failed || disabled_info.status == JobStatus::Failed {
        eprintln!("SKIP: server bilingual L2 morphotag test needs both English and Spanish models");
        return;
    }

    assert_eq!(
        enabled_info.status,
        JobStatus::Completed,
        "server L2-enabled morphotag run should complete"
    );
    assert_eq!(
        disabled_info.status,
        JobStatus::Completed,
        "server L2-disabled morphotag run should complete"
    );

    let enabled_file = &enabled_results.files[0];
    let disabled_file = &disabled_results.files[0];

    assert!(
        enabled_file.error.is_none() && disabled_file.error.is_none(),
        "both server L2 runs should complete cleanly"
    );

    let tienda_on = find_mor_line_for(&enabled_file.content, "tienda@s:spa")
        .expect("tienda MOR with server L2 enabled");
    let tienda_off = find_mor_line_for(&disabled_file.content, "tienda@s:spa")
        .expect("tienda MOR with server L2 disabled");

    assert!(
        !tienda_on.contains("L2|xxx"),
        "server L2-enabled run should splice real morphology, got: {tienda_on}"
    );
    assert!(
        tienda_off.contains("L2|xxx"),
        "server L2-disabled run should preserve L2|xxx fallback, got: {tienda_off}"
    );
    assert_ne!(
        enabled_file.content, disabled_file.content,
        "server L2 opt-out should materially change bilingual output"
    );
}

/// Verify unsupported inline-language fallback stays local to the affected
/// file on the server path, instead of poisoning neighboring file results.
#[tokio::test]
async fn morphotag_server_unsupported_inline_language_does_not_poison_neighbor_file() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let initial = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            vec![
                FilePayload {
                    filename: "server_clean.cha".into(),
                    content: ENG_SIMPLE_SERVER.into(),
                },
                FilePayload {
                    filename: "server_unsupported.cha".into(),
                    content: ENG_XYZ_L2.into(),
                },
            ],
            CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    batch_window: 0,
                    ..CommonOptions::default()
                },

                ..Default::default()
            }),
        )
        .await;

    let final_info = jobs.poll_done(&initial.job_id).await;
    assert_eq!(
        final_info.status,
        JobStatus::Completed,
        "mixed server morphotag job should complete"
    );

    let results = jobs.job_results(&initial.job_id).await;
    assert_eq!(
        results.files.len(),
        2,
        "server job should return both file results"
    );

    let clean = results
        .files
        .iter()
        .find(|file| file.filename == "server_clean.cha")
        .expect("clean server result");
    let unsupported = results
        .files
        .iter()
        .find(|file| file.filename == "server_unsupported.cha")
        .expect("unsupported server result");

    assert!(
        clean.error.is_none(),
        "clean server file should not inherit an error"
    );
    assert!(
        unsupported.error.is_none(),
        "unsupported-inline file should still complete with fallback"
    );
    assert!(
        clean.content.contains("%mor:"),
        "clean neighbor should still receive morphology on the server path"
    );
    assert!(
        !clean.content.contains("L2|xxx"),
        "clean neighbor should not inherit unsupported-language fallback"
    );

    let blorx_mor = find_mor_line_for(&unsupported.content, "blorx@s:xyz")
        .expect("unsupported inline file should still have a MOR line");
    assert!(
        blorx_mor.contains("L2|xxx"),
        "unsupported inline fallback should stay on the affected server file, got: {blorx_mor}"
    );
}

/// Verify `skipmultilang` keeps working through the server path at utterance
/// granularity rather than suppressing whole neighboring files.
#[tokio::test]
async fn morphotag_server_skipmultilang_only_skips_non_primary_utterances() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let initial = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            vec![
                FilePayload {
                    filename: "server_mono.cha".into(),
                    content: ENG_SIMPLE_SERVER.into(),
                },
                FilePayload {
                    filename: "server_bilingual.cha".into(),
                    content: ENG_SPA_PRECODE.into(),
                },
            ],
            CommandOptions::Morphotag(MorphotagOptions {
                common: CommonOptions {
                    override_media_cache: true,
                    batch_window: 0,
                    ..CommonOptions::default()
                },
                skipmultilang: true,

                ..Default::default()
            }),
        )
        .await;

    let final_info = jobs.poll_done(&initial.job_id).await;
    if final_info.status == JobStatus::Failed {
        eprintln!(
            "SKIP: server skipmultilang differential test needs both English and Spanish models"
        );
        return;
    }
    assert_eq!(
        final_info.status,
        JobStatus::Completed,
        "server skipmultilang morphotag job should complete"
    );

    let results = jobs.job_results(&initial.job_id).await;
    assert_eq!(
        results.files.len(),
        2,
        "server job should return both file results"
    );

    let mono = results
        .files
        .iter()
        .find(|file| file.filename == "server_mono.cha")
        .expect("monolingual server result");
    let bilingual = results
        .files
        .iter()
        .find(|file| file.filename == "server_bilingual.cha")
        .expect("bilingual server result");

    assert!(
        mono.error.is_none(),
        "monolingual server file should complete cleanly"
    );
    assert!(
        bilingual.error.is_none(),
        "bilingual server file should complete cleanly under skipmultilang"
    );
    assert_eq!(
        count_mor_lines(&mono.content),
        2,
        "monolingual server neighbor should keep both utterances tagged"
    );
    assert_eq!(
        count_mor_lines(&bilingual.content),
        1,
        "server skipmultilang should skip only the non-primary [- spa] utterance"
    );
    assert!(
        bilingual.content.contains("*CHI:\t[- spa] hola mundo ."),
        "server output should retain the skipped bilingual utterance"
    );
}
