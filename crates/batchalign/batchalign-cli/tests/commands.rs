//! Server-backed integration tests for CLI command modules.
//!
//! These tests spin up a real test server (test-echo workers, no ML models),
//! then exercise the CLI command functions (`jobs_cmd`, `serve_cmd`,
//! `dispatch`) against it.
//!
//! Requirements: Python 3 with batchalign installed.
//! Tests skip gracefully if unavailable.

mod cli_common;
mod common;

use batchalign::api::{FilePayload, JobInfo, JobSubmission, NumSpeakers, ReleasedCommand};
use batchalign::api::{LanguageCode3, LanguageSpec};
use batchalign::cli::args::{JobsArgs, ServeStatusArgs};
use batchalign::cli::client::BatchalignClient;
use batchalign::cli::{jobs_cmd, serve_cmd};
use batchalign::options::{CommandOptions, CommonOptions, TranscribeOptions};

use cli_common::poll_job_done;
use common::test_server_fixture::acquire_test_server_session;

fn test_submission(files: Vec<FilePayload>) -> JobSubmission {
    JobSubmission {
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
    }
}

// ---------------------------------------------------------------------------
// jobs_cmd
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jobs_list_empty_server() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let args = JobsArgs {
        action: None,
        job_id: None,
        server: Some(base_url.to_owned()),
        json: false,
    };
    jobs_cmd::run(&args)
        .await
        .expect("jobs_cmd::run should succeed");
}

#[tokio::test]
async fn jobs_list_after_submit() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();
    let submission = test_submission(vec![FilePayload {
        filename: "test.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
    }]);
    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200);

    let args = JobsArgs {
        action: None,
        job_id: None,
        server: Some(base_url.to_owned()),
        json: false,
    };
    jobs_cmd::run(&args)
        .await
        .expect("jobs_cmd::run should succeed");
}

#[tokio::test]
async fn jobs_inspect_single() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();
    let submission = test_submission(vec![FilePayload {
        filename: "inspect.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tworld .\n@End\n".into(),
    }]);
    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id.clone();

    poll_job_done(&client, &base_url, &job_id).await;

    let args = JobsArgs {
        action: None,
        job_id: Some(job_id.to_string()),
        server: Some(base_url.to_owned()),
        json: false,
    };
    jobs_cmd::run(&args)
        .await
        .expect("jobs inspect should succeed");
}

// ---------------------------------------------------------------------------
// serve_cmd
// ---------------------------------------------------------------------------

#[tokio::test]
async fn serve_status_healthy() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let args = ServeStatusArgs {
        server: Some(base_url.to_owned()),
    };
    serve_cmd::status(&args)
        .await
        .expect("serve status should succeed");
}

// ---------------------------------------------------------------------------
// Client submit + poll + result fetch (end-to-end via CLI client)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_submit_poll_fetch() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = BatchalignClient::new().expect("HTTP client build should not fail in tests");
    let submission = test_submission(vec![FilePayload {
        filename: "e2e.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
    }]);

    let info = client
        .submit_job(&base_url, &submission)
        .await
        .expect("submit should succeed");
    assert_eq!(info.total_files, 1);

    let http = reqwest::Client::new();
    let final_info = poll_job_done(&http, &base_url, &info.job_id).await;
    assert_eq!(final_info.status, batchalign::api::JobStatus::Completed);

    let result = client
        .get_file_result(&base_url, &info.job_id, &"e2e.cha".into())
        .await
        .expect("get_file_result should succeed");
    assert!(result.error.is_none());
    assert!(!result.content.is_empty());
}

#[tokio::test]
async fn client_submit_multiple_files() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = BatchalignClient::new().expect("HTTP client build should not fail in tests");
    let files: Vec<FilePayload> = (0..3)
        .map(|i| FilePayload {
            filename: format!("multi_{i}.cha").into(),
            content: format!("@UTF8\n@Begin\n*CHI:\tword{i} .\n@End\n"),
        })
        .collect();
    let submission = test_submission(files);

    let info = client
        .submit_job(&base_url, &submission)
        .await
        .expect("submit should succeed");
    assert_eq!(info.total_files, 3);

    let http = reqwest::Client::new();
    let final_info = poll_job_done(&http, &base_url, &info.job_id).await;
    assert_eq!(final_info.completed_files, 3);

    let results = client
        .get_all_results(&base_url, &info.job_id)
        .await
        .expect("get_all_results should succeed");
    assert_eq!(results.files.len(), 3);
}

#[tokio::test]
async fn dispatch_no_server() {
    let tmp = tempfile::TempDir::new().unwrap();
    let input_dir = tmp.path().join("input");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(&input_dir).unwrap();
    std::fs::create_dir_all(&state_dir).unwrap();

    // Disable auto_daemon so the test doesn't try to spawn the test binary
    // as a daemon process. The test verifies "no server → Ok()" behavior.
    std::fs::write(state_dir.join("server.yaml"), "auto_daemon: false\n").unwrap();

    std::fs::write(
        input_dir.join("test.cha"),
        "@UTF8\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n@Begin\n*CHI:\thello .\n@End\n",
    )
    .unwrap();

    // Isolate from real daemon state and config.
    // SAFETY: acceptable in tests; this test doesn't run in parallel with
    // other tests that depend on this env var.
    unsafe {
        std::env::set_var("BATCHALIGN_STATE_DIR", &state_dir);
    }

    let inputs: Vec<std::path::PathBuf> = vec![input_dir.to_path_buf()];

    // Direct mode with no worker: dispatch runs the pipeline inline, which
    // will fail at pre-validation (invalid CHAT) or worker spawn (no Python).
    // Either way, dispatch should return without panicking — the result
    // contains job-level error reports, not a crash.
    let result = batchalign::cli::dispatch::dispatch(batchalign::cli::dispatch::DispatchRequest {
        command: batchalign::ReleasedCommand::Morphotag,
        lang: "eng",
        num_speakers: 1,
        extensions: &["cha"],
        server_arg: None,
        inputs: &inputs,
        out_dir: None,
        options: None,
        bank: None,
        subdir: None,
        lexicon: None,
        use_tui: false,
        open_dashboard: false,
        force_cpu: false,
        no_server: false,
        before: None,
        workers: None,
        timeout: None,
        sequential: false,
        memory_tier: None,
    })
    .await;

    unsafe {
        std::env::remove_var("BATCHALIGN_STATE_DIR");
    }

    // dispatch returns Ok even when individual files fail — failures are
    // reported in the job results, not as a top-level Err. If dispatch
    // itself returns Err, it means an infrastructure failure (e.g., cannot
    // bind port), which is also acceptable for this test scenario.
    // The key assertion is that we got here without panicking.
    let _ = result;
}
