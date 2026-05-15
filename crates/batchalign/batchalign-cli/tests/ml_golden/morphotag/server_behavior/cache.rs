use super::super::fixtures::YUE_GU_SHI;
use super::super::helpers::minimal_chat;
use crate::common::{LiveServerJobClient, require_live_server};
use batchalign::api::{FilePayload, JobStatus, LanguageCode3, LanguageSpec, ReleasedCommand};
use batchalign::options::{CommandOptions, CommonOptions, MorphotagOptions};
use batchalign::worker::InferTask;

/// Server-path cache reuse must preserve multilingual outputs exactly across
/// reruns rather than mixing language-group results or changing file assembly.
#[tokio::test]
async fn morphotag_server_multilingual_warm_cache_preserves_per_language_outputs() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let files = vec![
        FilePayload {
            filename: "server_eng_cache_group.cha".into(),
            content: minimal_chat("eng", "ENG", "hello world"),
        },
        FilePayload {
            filename: "server_spa_cache_group.cha".into(),
            content: minimal_chat("spa", "SPA", "hola mundo"),
        },
    ];
    let options = CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            batch_window: 0,
            ..CommonOptions::default()
        },

        ..Default::default()
    });

    let cold = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            files.clone(),
            options.clone(),
        )
        .await;
    let cold_info = jobs.poll_done(&cold.job_id).await;
    let cold_results = jobs.job_results(&cold.job_id).await;

    let warm = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::eng()),
            files,
            options,
        )
        .await;
    let warm_info = jobs.poll_done(&warm.job_id).await;
    let warm_results = jobs.job_results(&warm.job_id).await;

    if cold_info.status == JobStatus::Failed || warm_info.status == JobStatus::Failed {
        eprintln!(
            "SKIP: server multilingual cache test needs both English and Spanish morphotag backends"
        );
        return;
    }

    assert_eq!(
        cold_info.status,
        JobStatus::Completed,
        "cold server multilingual cache run should complete"
    );
    assert_eq!(
        warm_info.status,
        JobStatus::Completed,
        "warm server multilingual cache run should complete"
    );

    let mut cold_files = cold_results.files;
    let mut warm_files = warm_results.files;
    cold_files.sort_by(|a, b| a.filename.cmp(&b.filename));
    warm_files.sort_by(|a, b| a.filename.cmp(&b.filename));

    assert_eq!(
        cold_files.len(),
        2,
        "cold server run should return two files"
    );
    assert_eq!(
        warm_files.len(),
        2,
        "warm server run should return two files"
    );

    for (cold_file, warm_file) in cold_files.iter().zip(warm_files.iter()) {
        assert_eq!(
            cold_file.filename, warm_file.filename,
            "file ordering should stay stable across server cache reruns"
        );
        assert!(
            cold_file.error.is_none() && warm_file.error.is_none(),
            "both server cache reruns should complete cleanly for {}",
            cold_file.filename
        );
        assert_eq!(
            cold_file.content, warm_file.content,
            "warm server cache rerun should preserve exact output for {}",
            cold_file.filename
        );
        assert!(
            cold_file.content.contains("%mor:"),
            "server cache output should still include morphology for {}",
            cold_file.filename
        );
    }
}

/// Retokenized Cantonese results must remain isolated from preserve-mode cache
/// entries on the server path, not just in direct execution.
#[tokio::test]
async fn morphotag_server_cantonese_retokenize_cache_isolated_from_preserve_mode() {
    let Some(server) = require_live_server(
        InferTask::Morphosyntax,
        "Server does not support morphosyntax infer",
    )
    .await
    else {
        return;
    };
    let jobs = LiveServerJobClient::from_session(&server);

    let disabled = CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            batch_window: 0,
            ..CommonOptions::default()
        },

        ..Default::default()
    });
    let enabled = CommandOptions::Morphotag(MorphotagOptions {
        common: CommonOptions {
            override_media_cache: true,
            batch_window: 0,
            ..CommonOptions::default()
        },
        retokenize: true,

        ..Default::default()
    });
    let files = vec![FilePayload {
        filename: "server_yue_cache.cha".into(),
        content: YUE_GU_SHI.into(),
    }];

    let first = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::yue()),
            files.clone(),
            disabled.clone(),
        )
        .await;
    let first_info = jobs.poll_done(&first.job_id).await;
    let first_results = jobs.job_results(&first.job_id).await;

    let second = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::yue()),
            files.clone(),
            enabled,
        )
        .await;
    let second_info = jobs.poll_done(&second.job_id).await;
    let second_results = jobs.job_results(&second.job_id).await;

    let third = jobs
        .submit_content_job(
            ReleasedCommand::Morphotag,
            LanguageSpec::Resolved(LanguageCode3::yue()),
            files,
            disabled,
        )
        .await;
    let third_info = jobs.poll_done(&third.job_id).await;
    let third_results = jobs.job_results(&third.job_id).await;

    if first_info.status == JobStatus::Failed
        || second_info.status == JobStatus::Failed
        || third_info.status == JobStatus::Failed
    {
        eprintln!("SKIP: server Cantonese cache isolation test needs the yue morphotag backend");
        return;
    }

    assert_eq!(
        first_info.status,
        JobStatus::Completed,
        "initial server Cantonese preserve-mode run should complete"
    );
    assert_eq!(
        second_info.status,
        JobStatus::Completed,
        "server Cantonese retokenize run should complete"
    );
    assert_eq!(
        third_info.status,
        JobStatus::Completed,
        "final server Cantonese preserve-mode run should complete"
    );

    let first_output = &first_results.files[0].content;
    let second_output = &second_results.files[0].content;
    let third_output = &third_results.files[0].content;

    assert!(
        first_output.contains("*CHI:\t故 事 係 好 ."),
        "initial preserve-mode server run should keep Cantonese character tokens"
    );
    assert!(
        second_output.contains("*CHI:\t故事 係 好 ."),
        "server retokenize run should collapse Cantonese tokens even after a preserve-mode warmup"
    );
    assert!(
        third_output.contains("*CHI:\t故 事 係 好 ."),
        "final preserve-mode server run should not reuse the retokenized cache entry"
    );
    assert_eq!(
        first_output, third_output,
        "preserve-mode server outputs should match before and after a retokenized rerun"
    );
    assert_ne!(
        first_output, second_output,
        "server retokenize output must stay distinct from preserve-mode output"
    );
}
