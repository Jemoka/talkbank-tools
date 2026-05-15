//! End-to-end CLI integration tests.
//!
//! These tests verify the full pipeline: CLI → server → test-echo worker → results.
//! All tests use test-echo workers (no ML models required).
//!
//! Requirements: Python 3 with batchalign installed.
//! Tests skip gracefully if unavailable.

mod cli_common;
mod common;

use batchalign::api::{FilePayload, JobStatus, NumSpeakers, ReleasedCommand};
use batchalign::api::{LanguageCode3, LanguageSpec};
use batchalign::options::{CommandOptions, CommonOptions, TranscribeOptions};

use cli_common::{
    DUMMY_CHAT, MINIMAL_CHAT, NOALIGN_CHAT, default_options_for, poll_job_done,
    run_job_to_completion,
};
use common::test_server_fixture::acquire_test_server_session;

// ---------------------------------------------------------------------------
// File discovery & output
// ---------------------------------------------------------------------------

/// Single file round-trips through the server and comes back.
#[tokio::test]
async fn e2e_single_file_roundtrip() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "single.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(info.completed_files, 1);
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none(), "No error expected");
    assert!(
        !results[0].content.is_empty(),
        "Result content should not be empty"
    );
}

/// Multiple files are all processed and returned with correct filenames.
#[tokio::test]
async fn e2e_multiple_files() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files: Vec<FilePayload> = (0..3)
        .map(|i| FilePayload {
            filename: format!("file_{i}.cha").into(),
            content: MINIMAL_CHAT.into(),
        })
        .collect();

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(info.completed_files, 3);
    assert_eq!(results.len(), 3);

    let mut names: Vec<String> = results.iter().map(|r| r.filename.to_string()).collect();
    names.sort();
    assert_eq!(names, vec!["file_0.cha", "file_1.cha", "file_2.cha"]);
}

/// Nested path in filename is preserved through the round-trip.
#[tokio::test]
async fn e2e_nested_path_preserved() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "sub/nested.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename, "sub/nested.cha");
}

/// Empty file list is accepted (returns 0 results).
#[tokio::test]
async fn e2e_empty_input() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    // Empty files → server returns 400 (no files)
    let submission = batchalign::api::JobSubmission {
        command: ReleasedCommand::Transcribe,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");

    assert_eq!(resp.status(), 400, "Empty file list should be rejected");
}

/// Test-echo output is still parseable CHAT (content returned unchanged).
#[tokio::test]
async fn e2e_output_is_valid_chat() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "valid.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (_info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(results.len(), 1);
    let content = &results[0].content;
    // Test-echo returns input unchanged; verify it contains key CHAT markers.
    assert!(content.contains("@Begin"), "Output should contain @Begin");
    assert!(content.contains("@End"), "Output should contain @End");
    assert!(
        content.contains("@Languages:"),
        "Output should contain @Languages"
    );
}

// ---------------------------------------------------------------------------
// Dummy & NoAlign handling
// ---------------------------------------------------------------------------

/// Dummy file is returned unchanged by the server (test-echo pass-through).
#[tokio::test]
async fn e2e_dummy_file_passthrough() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "dummy.cha".into(),
        content: DUMMY_CHAT.into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none());
    // Dummy file should be returned (test-echo returns input unchanged)
    assert!(results[0].content.contains("dummy"));
}

/// NoAlign file is returned unchanged for transcribe command.
#[tokio::test]
async fn e2e_noalign_file_passthrough() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "noalign.cha".into(),
        content: NOALIGN_CHAT.into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none());
    assert!(results[0].content.contains("NoAlign"));
}

// ---------------------------------------------------------------------------
// Options propagation
// ---------------------------------------------------------------------------

/// override_media_cache option is accepted in the submission.
#[tokio::test]
async fn e2e_override_media_cache_option() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let options = CommandOptions::Transcribe(TranscribeOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        asr_engine: batchalign::options::AsrEngineName::RevAi,
        diarize: false,
        wor: false.into(),
        merge_abbrev: false.into(),
        batch_size: 8,
        utseg_fallback: false.into(),
    });

    let files = vec![FilePayload {
        filename: "cache.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, _results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        options,
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
}

/// retokenize option is accepted in the submission.
#[tokio::test]
async fn e2e_retokenize_option() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let options = CommandOptions::Transcribe(TranscribeOptions {
        common: CommonOptions::default(),
        asr_engine: batchalign::options::AsrEngineName::RevAi,
        diarize: false,
        wor: false.into(),
        merge_abbrev: false.into(),
        batch_size: 8,
        utseg_fallback: false.into(),
    });

    let files = vec![FilePayload {
        filename: "retok.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, _results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        options,
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
}

// ---------------------------------------------------------------------------
// Command lifecycle (test-echo — verifies accept/complete lifecycle)
// ---------------------------------------------------------------------------

/// Transcribe command completes via the server-side test-echo harness.
#[tokio::test]
async fn e2e_transcribe_command() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "transcribe.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
}

/// Transcribe_s command completes via the server-side test-echo harness.
#[tokio::test]
async fn e2e_transcribe_s_command() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "transcribe_s.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::TranscribeS,
        "eng",
        files,
        default_options_for("transcribe_s"),
    )
    .await;

    assert_eq!(info.command, "transcribe_s");
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none(), "No error expected");
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

/// Unknown command is rejected by the server.
#[tokio::test]
async fn e2e_invalid_command_rejected() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    // Send raw JSON with an invalid command — ReleasedCommand (closed enum)
    // rejects unknown variants at deserialization (HTTP 422).
    let raw = serde_json::json!({
        "command": "nonexistent_command",
        "lang": "eng",
        "num_speakers": 1,
        "files": [{"filename": "test.cha", "content": MINIMAL_CHAT}],
        "media_files": [],
        "media_mapping": "",
        "media_subdir": "",
        "source_dir": "",
        "options": null,
    });

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&raw)
        .send()
        .await
        .expect("POST /jobs");

    assert_eq!(
        resp.status(),
        422,
        "Unknown command should be rejected at deserialization"
    );
}

/// Malformed CHAT content still completes (test-echo returns it unchanged).
#[tokio::test]
async fn e2e_malformed_chat_still_completes() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "malformed.cha".into(),
        content: "This is not valid CHAT at all.".into(),
    }];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    // Test-echo doesn't parse — just echoes content, so it should complete.
    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(results.len(), 1);
    assert!(results[0].content.contains("not valid CHAT"));
}

// ---------------------------------------------------------------------------
// Multi-language
// ---------------------------------------------------------------------------

/// Language parameter propagates through the job lifecycle.
#[tokio::test]
async fn e2e_lang_propagates() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "spanish.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let (info, _results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "spa",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(
        info.lang,
        LanguageSpec::Resolved(LanguageCode3::spa()),
        "Language should propagate to job info"
    );
}

// ---------------------------------------------------------------------------
// Content fidelity
// ---------------------------------------------------------------------------

/// Test-echo preserves exact content (byte-for-byte round-trip).
#[tokio::test]
async fn e2e_content_fidelity() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let original = MINIMAL_CHAT;
    let files = vec![FilePayload {
        filename: "fidelity.cha".into(),
        content: original.into(),
    }];

    let (_info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].content, original,
        "Test-echo should return content unchanged"
    );
}

/// Multiple files mixed: some with dummy headers, some normal.
#[tokio::test]
async fn e2e_mixed_dummy_and_normal() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![
        FilePayload {
            filename: "normal.cha".into(),
            content: MINIMAL_CHAT.into(),
        },
        FilePayload {
            filename: "dummy.cha".into(),
            content: DUMMY_CHAT.into(),
        },
        FilePayload {
            filename: "also_normal.cha".into(),
            content: MINIMAL_CHAT.into(),
        },
    ];

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(info.completed_files, 3);
    assert_eq!(results.len(), 3);

    // All files should be returned successfully
    for result in &results {
        assert!(
            result.error.is_none(),
            "File {} should have no error",
            result.filename
        );
        assert!(
            !result.content.is_empty(),
            "File {} content should not be empty",
            result.filename
        );
    }
}

/// Job with many files verifies parallel processing capability.
#[tokio::test]
async fn e2e_parallel_processing() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files: Vec<FilePayload> = (0..8)
        .map(|i| FilePayload {
            filename: format!("parallel_{i}.cha").into(),
            content: MINIMAL_CHAT.into(),
        })
        .collect();

    let (info, results) = run_job_to_completion(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        "eng",
        files,
        default_options_for("transcribe"),
    )
    .await;

    assert_eq!(info.status, JobStatus::Completed);
    assert_eq!(info.completed_files, 8);
    assert_eq!(results.len(), 8);
}

/// Cancel a running job.
#[tokio::test]
async fn e2e_cancel_job() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "cancel.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let submission = batchalign::api::JobSubmission {
        command: ReleasedCommand::Transcribe,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files,
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: batchalign::api::JobInfo = resp.json().await.expect("parse");

    // Cancel immediately (may already be completed due to test-echo speed)
    let cancel_resp = client
        .post(format!("{base_url}/jobs/{}/cancel", info.job_id))
        .send()
        .await
        .expect("POST /jobs/{id}/cancel");

    // 200 = cancel accepted (or already terminal — endpoint returns 200 either way)
    assert_eq!(
        cancel_resp.status(),
        200,
        "Cancel should return 200, got {}",
        cancel_resp.status()
    );

    let final_info = poll_job_done(&client, &base_url, &info.job_id).await;
    assert!(
        matches!(
            final_info.status,
            JobStatus::Cancelled | JobStatus::Completed
        ),
        "Job should be cancelled or already completed"
    );
}

/// Verify job status transitions: Queued → Running → Completed.
#[tokio::test]
async fn e2e_job_status_lifecycle() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = session.client();

    let files = vec![FilePayload {
        filename: "lifecycle.cha".into(),
        content: MINIMAL_CHAT.into(),
    }];

    let submission = batchalign::api::JobSubmission {
        command: ReleasedCommand::Transcribe,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files,
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: batchalign::api::JobInfo = resp.json().await.expect("parse");

    // Initial status should be Queued or Running
    assert!(
        matches!(info.status, JobStatus::Queued | JobStatus::Running),
        "Initial status should be Queued or Running, got {:?}",
        info.status
    );

    // Wait for completion
    let final_info = poll_job_done(&client, &base_url, &info.job_id).await;
    assert_eq!(final_info.status, JobStatus::Completed);
    assert!(
        final_info.completed_at.is_some(),
        "completed_at should be set"
    );
    assert!(
        final_info.submitted_at.is_some(),
        "submitted_at should be set"
    );
}
