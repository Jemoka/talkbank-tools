// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Smoke tests that exercise every released command family through the full
//! HTTP job lifecycle: submit → poll → verify results.
//!
//! Each test submits a job for one command family using test-echo workers (no
//! ML models), verifies the job completes, and checks that result files are
//! present. This proves the dispatch routing, job store persistence, and
//! result retrieval work end-to-end for every command.
//!
//! These are Tier 1 (fast) tests — no real inference, safe to run on any machine.

mod common;

use batchalign::api::{
    FilePayload, JobInfo, JobResultResponse, JobStatus, LanguageCode3, LanguageSpec, NumSpeakers,
    ReleasedCommand,
};
use batchalign::options::{
    AlignOptions, AsrEngineName, CommandOptions, CommonOptions, CorefOptions, MorphotagOptions,
    TranscribeOptions, TranslateOptions, UtsegOptions,
};
use common::test_server_fixture::acquire_test_server_session;

use batchalign::api::JobSubmission;

/// Poll until a job reaches a terminal state (completed, failed, cancelled).
async fn poll_job_done(client: &reqwest::Client, base_url: &str, job_id: &str) -> JobInfo {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(60);
    loop {
        let resp = client
            .get(format!("{base_url}/jobs/{job_id}"))
            .send()
            .await
            .expect("GET job");
        let body = resp.text().await.expect("read body");
        let info: JobInfo = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("parse job failed: {e}\nbody: {body}"));

        if matches!(
            info.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return info;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Job {job_id} did not finish within 60s (status: {:?})",
            info.status
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}

fn minimal_chat() -> String {
    // Include `@Languages: eng` so commands using `LanguageSpec::PerFile`
    // (morphotag, translate, coref) can resolve a language from the
    // CHAT header. Without this, the per-file resolver fails with a
    // missing-header error and the smoke test never reaches dispatch.
    "@UTF8\n@Languages:\teng\n@Begin\n*CHI:\thello world .\n@End\n".into()
}

/// Pick the legal `LanguageSpec` for each command — morphotag,
/// translate, and coref require `PerFile` (per the submission validator
/// at `crates/batchalign/src/types/request.rs::validate_lang_command_pairing`,
/// codified after the 2026-05-03 morphotag incident); every other
/// command requires `Auto` or `Resolved`. The smoke tests previously
/// hardcoded `Resolved(eng())` for all commands, which made the three
/// per-file-lang tests fail at the validation boundary before dispatch
/// ever ran.
fn lang_for_command(command: ReleasedCommand) -> LanguageSpec {
    match command {
        ReleasedCommand::Morphotag | ReleasedCommand::Translate | ReleasedCommand::Coref => {
            LanguageSpec::PerFile
        }
        _ => LanguageSpec::Resolved(LanguageCode3::eng()),
    }
}

fn make_submission(command: ReleasedCommand, options: CommandOptions) -> JobSubmission {
    JobSubmission {
        command,
        lang: lang_for_command(command),
        num_speakers: NumSpeakers(1),
        files: vec![FilePayload {
            filename: "smoke.cha".into(),
            content: minimal_chat(),
        }],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    }
}

/// Submit a job with the given command and options, verify it completes,
/// and return the result response.
async fn submit_and_verify(
    client: &reqwest::Client,
    base_url: &str,
    command: ReleasedCommand,
    options: CommandOptions,
) -> JobResultResponse {
    let submission = make_submission(command, options);
    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(
        resp.status(),
        200,
        "submit {command:?} failed: {}",
        resp.text().await.unwrap_or_default()
    );

    let info: JobInfo = {
        let body = resp.text().await.expect("read submit body");
        serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("parse submit response for {command:?}: {e}\n{body}"))
    };
    assert_eq!(info.command, command);

    let final_info = poll_job_done(client, base_url, &info.job_id).await;
    assert!(
        matches!(final_info.status, JobStatus::Completed),
        "{command:?} job did not complete: {:?}",
        final_info.status
    );

    let resp = client
        .get(format!("{base_url}/jobs/{}/results", info.job_id))
        .send()
        .await
        .expect("GET results");
    assert_eq!(resp.status(), 200);
    let results: JobResultResponse = resp.json().await.expect("parse results");
    assert!(
        !results.files.is_empty(),
        "{command:?} produced no result files"
    );
    results
}

// ---------------------------------------------------------------------------
// Command family smoke tests
// ---------------------------------------------------------------------------

/// Morphotag: batched text inference (Stanza profile).
#[tokio::test]
async fn smoke_morphotag() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = reqwest::Client::new();

    let results = submit_and_verify(
        &client,
        &base_url,
        ReleasedCommand::Morphotag,
        CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),
            merge_abbrev: Default::default(),

            ..Default::default()
        }),
    )
    .await;

    assert!(
        results.files[0].error.is_none(),
        "morphotag smoke file should have no error"
    );
}

/// Utseg: batched text inference (Stanza constituency profile).
#[tokio::test]
async fn smoke_utseg() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = reqwest::Client::new();

    let results = submit_and_verify(
        &client,
        &base_url,
        ReleasedCommand::Utseg,
        CommandOptions::Utseg(UtsegOptions {
            common: CommonOptions::default(),
            merge_abbrev: Default::default(),
            utseg_fallback: false.into(),
        }),
    )
    .await;

    assert!(
        results.files[0].error.is_none(),
        "utseg smoke file should have no error"
    );
}

/// Translate: batched text inference (IO profile).
#[tokio::test]
async fn smoke_translate() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = reqwest::Client::new();

    let results = submit_and_verify(
        &client,
        &base_url,
        ReleasedCommand::Translate,
        CommandOptions::Translate(TranslateOptions {
            common: CommonOptions::default(),
            merge_abbrev: Default::default(),
        }),
    )
    .await;

    assert!(
        results.files[0].error.is_none(),
        "translate smoke file should have no error"
    );
}

/// Coref: batched text inference (Stanza coref profile).
#[tokio::test]
async fn smoke_coref() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = reqwest::Client::new();

    let results = submit_and_verify(
        &client,
        &base_url,
        ReleasedCommand::Coref,
        CommandOptions::Coref(CorefOptions {
            common: CommonOptions::default(),
            merge_abbrev: Default::default(),
        }),
    )
    .await;

    assert!(
        results.files[0].error.is_none(),
        "coref smoke file should have no error"
    );
}

/// Transcribe: per-file audio pipeline (GPU profile for ASR + speaker).
/// Test-echo workers process this without real audio.
#[tokio::test]
async fn smoke_transcribe() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = reqwest::Client::new();

    let results = submit_and_verify(
        &client,
        &base_url,
        ReleasedCommand::Transcribe,
        CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: Default::default(),
            merge_abbrev: Default::default(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
    )
    .await;

    // Transcribe returns content (test-echo echoes the input).
    assert!(
        !results.files[0].content.is_empty(),
        "transcribe smoke should return content"
    );
}

/// Align: per-file audio pipeline (GPU profile for FA).
/// Test-echo workers process this without real audio.
#[tokio::test]
async fn smoke_align() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();
    let client = reqwest::Client::new();

    let results = submit_and_verify(
        &client,
        &base_url,
        ReleasedCommand::Align,
        CommandOptions::Align(AlignOptions::default()),
    )
    .await;

    assert!(
        !results.files[0].content.is_empty(),
        "align smoke should return content"
    );
}
