// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Integration tests for batchalign-server.
//!
//! These tests spin up a real HTTP server with test-echo workers
//! (no ML models), submit jobs, and verify responses.
//!
//! Requirements: Python 3 with batchalign installed.
//! Tests skip gracefully if unavailable.

mod common;

use batchalign::api::{
    FilePayload, FileResult, HealthResponse, HealthStatus, JobControlPlaneBackendKind, JobInfo,
    JobListItem, JobResultResponse, JobStatus, JobSubmission, LanguageCode3, LanguageSpec,
    MemoryMb, NumSpeakers, ReleasedCommand,
};
use batchalign::config::ServerConfig;
use batchalign::create_test_app;
use batchalign::host_facts::PerProfile;
use batchalign::options::{CommandOptions, CommonOptions, TranscribeOptions};
use batchalign::worker::pool::PoolConfig;
use common::resolve_python;
use common::test_server_fixture::{
    acquire_test_server_session, acquire_test_server_session_with_config,
    isolate_host_memory_ledger,
};

/// Skip a real-worker test (test_echo=false) when Python is unavailable.
/// The shared test-echo fixture handles SKIP for the test-echo path; this
/// macro covers the one capability-gate test that intentionally bypasses
/// the shared fixture to exercise the non-test-echo capability probe.
macro_rules! require_python {
    () => {
        match resolve_python() {
            Some(path) => path,
            None => {
                eprintln!("SKIP: Python 3 with batchalign not available");
                return;
            }
        }
    };
}

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
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_endpoint() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let resp = reqwest::get(format!("{base_url}/health"))
        .await
        .expect("GET /health");
    assert_eq!(resp.status(), 200);

    let health: HealthResponse = resp.json().await.expect("parse health");
    assert_eq!(health.status, HealthStatus::Ok);
    assert!(!health.version.is_empty());
    assert!(!health.build_hash.is_empty());
    assert_eq!(health.cache_backend, "sqlite");
    assert!(health.capabilities.iter().any(|cap| cap == "transcribe"));
}

#[tokio::test]
async fn submit_and_get_job() {
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

    let info: JobInfo = resp.json().await.expect("parse job info");
    assert_eq!(info.command, ReleasedCommand::Transcribe);
    assert_eq!(info.lang, LanguageSpec::Resolved(LanguageCode3::eng()));
    assert_eq!(info.total_files, 1);
    let job_id = info.job_id.clone();

    // Poll until the job finishes (test-echo is fast)
    let info = poll_job_done(&client, &base_url, &job_id).await;
    assert!(
        matches!(info.status, JobStatus::Completed),
        "Expected Completed, got {:?}",
        info.status
    );
    assert_eq!(info.completed_files, 1);
    assert_eq!(
        info.control_plane.as_ref().map(|control| control.backend),
        Some(JobControlPlaneBackendKind::Test)
    );
}

#[tokio::test]
async fn submit_job_echoes_request_id_header() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();
    let submission = test_submission(vec![FilePayload {
        filename: "hdr.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .header("x-request-id", "external-trace-123")
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200);

    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(req_id, "external-trace-123");
}

#[tokio::test]
async fn submit_job_sets_request_id_header_when_missing() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();
    let submission = test_submission(vec![FilePayload {
        filename: "fallback.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200);

    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(!req_id.is_empty());

    let info: JobInfo = resp.json().await.expect("parse job info");
    assert_eq!(req_id, info.job_id.to_string());
}

// Plugin command test removed: with ReleasedCommand (closed enum), unknown
// commands are rejected at JSON deserialization. See unknown_command_returns_422.

#[tokio::test]
async fn submit_and_get_results() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let content = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
    let submission = test_submission(vec![FilePayload {
        filename: "result_test.cha".into(),
        content: content.into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    // Wait for completion
    poll_job_done(&client, &base_url, &job_id).await;

    // Get results
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/results"))
        .send()
        .await
        .expect("GET /jobs/{id}/results");
    assert_eq!(resp.status(), 200);

    let results: JobResultResponse = resp.json().await.expect("parse results");
    assert_eq!(results.job_id, job_id);
    assert!(matches!(results.status, JobStatus::Completed));
    assert!(!results.files.is_empty());

    // Test-echo worker echoes input — the result should contain content
    let first = &results.files[0];
    assert!(first.error.is_none());
    assert!(!first.content.is_empty());
}

#[tokio::test]
async fn get_single_result() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "single.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tworld .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    poll_job_done(&client, &base_url, &job_id).await;

    // Get single file result
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/results/single.cha"))
        .send()
        .await
        .expect("GET single result");
    assert_eq!(resp.status(), 200);

    let result: FileResult = resp.json().await.expect("parse file result");
    assert!(result.error.is_none());
    assert!(!result.content.is_empty());
}

/// Filenames with slashes (e.g. `resources/corpus/subdir/file.cha`) must be retrievable
/// via `GET /jobs/{id}/results/{*filename}`.  Before the `{*filename}` wildcard
/// fix, axum interpreted the slashes as additional path segments and returned
/// 404.
#[tokio::test]
async fn get_single_result_with_slashes_in_filename() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let filename = "resources/corpus/subdir/deep/file.cha";
    let submission = test_submission(vec![FilePayload {
        filename: filename.into(),
        content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    poll_job_done(&client, &base_url, &job_id).await;

    // Fetch using the slashed filename — this was returning 404 before the fix.
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/results/{filename}"))
        .send()
        .await
        .expect("GET result with slashes");
    assert_eq!(
        resp.status(),
        200,
        "slashed filename should be retrievable via wildcard path"
    );

    let result: FileResult = resp.json().await.expect("parse file result");
    assert_eq!(&*result.filename, filename);
    assert!(result.error.is_none());
    assert!(!result.content.is_empty());
}

#[tokio::test]
async fn list_jobs() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Submit two jobs
    for name in &["list_a.cha", "list_b.cha"] {
        let sub = test_submission(vec![FilePayload {
            filename: (*name).into(),
            content: "@UTF8\n@Begin\n*CHI:\thi .\n@End\n".into(),
        }]);
        client
            .post(format!("{base_url}/jobs"))
            .json(&sub)
            .send()
            .await
            .expect("POST /jobs");
    }

    let resp = client
        .get(format!("{base_url}/jobs"))
        .send()
        .await
        .expect("GET /jobs");
    assert_eq!(resp.status(), 200);

    let jobs: Vec<JobListItem> = resp.json().await.expect("parse job list");
    assert!(jobs.len() >= 2);
}

#[tokio::test]
async fn cancel_job() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "cancel.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tcancel .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    // Cancel immediately
    let resp = client
        .post(format!("{base_url}/jobs/{job_id}/cancel"))
        .send()
        .await
        .expect("POST cancel");
    assert_eq!(resp.status(), 200);

    // Verify status is cancelled (or possibly already completed if echo was faster)
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}"))
        .send()
        .await
        .expect("GET job");
    let info: JobInfo = resp.json().await.expect("parse");
    assert!(matches!(
        info.status,
        JobStatus::Cancelled | JobStatus::Completed
    ));
}

/// RED test (Phase 1, RED 1.1) — provenance is captured on cancel.
///
/// Submits a job, POSTs `/jobs/{id}/cancel` with a JSON body declaring
/// the caller's identity, then verifies that:
///   1. `GET /jobs/{id}` exposes denormalized `last_cancelled_*` fields
///      reflecting the cancel request.
///   2. `GET /jobs/{id}/cancellations` returns the audit row with the
///      same metadata.
///
/// This test fails until:
///   - The `cancellations` audit table + `jobs.last_cancelled_*` columns
///     ship in the migration (done).
///   - The cancel route handler parses the body and persists provenance.
///   - The new `GET /jobs/{id}/cancellations` route is wired.
///   - `JobInfo` exposes the `last_cancelled_*` fields.
#[tokio::test]
async fn cancel_records_provenance() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "provenance.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tcancel .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id.clone();

    // Cancel WITH provenance body.
    let body = serde_json::json!({
        "source": "tui",
        "host": "test-laptop",
        "pid": 4242,
        "reason": "user-pressed-cancel",
        "in_flight_filename": "provenance.cha"
    });
    let resp = client
        .post(format!("{base_url}/jobs/{job_id}/cancel"))
        .json(&body)
        .send()
        .await
        .expect("POST cancel with body");
    assert_eq!(
        resp.status(),
        200,
        "cancel-with-body should succeed (route accepts optional JSON body)"
    );

    // Assertion 1: jobs row exposes denormalized last-cancelled fields.
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}"))
        .send()
        .await
        .expect("GET job");
    assert_eq!(resp.status(), 200);
    let raw_job: serde_json::Value = resp.json().await.expect("parse job json");
    assert_eq!(
        raw_job
            .get("last_cancelled_source")
            .and_then(|v| v.as_str()),
        Some("tui"),
        "jobs.last_cancelled_source must reflect the cancel-body source field; got {raw_job:?}"
    );
    assert_eq!(
        raw_job.get("last_cancelled_host").and_then(|v| v.as_str()),
        Some("test-laptop"),
        "jobs.last_cancelled_host must reflect the cancel-body host field"
    );
    assert_eq!(
        raw_job
            .get("last_cancelled_reason")
            .and_then(|v| v.as_str()),
        Some("user-pressed-cancel"),
        "jobs.last_cancelled_reason must reflect the cancel-body reason field"
    );
    assert!(
        raw_job
            .get("last_cancelled_at")
            .and_then(|v| v.as_f64())
            .is_some(),
        "jobs.last_cancelled_at must be populated"
    );

    // Assertion 2: cancellations audit endpoint returns the recorded row.
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/cancellations"))
        .send()
        .await
        .expect("GET /jobs/{id}/cancellations");
    assert_eq!(
        resp.status(),
        200,
        "GET /jobs/{{id}}/cancellations endpoint must exist"
    );
    let audit: Vec<serde_json::Value> = resp.json().await.expect("parse audit list");
    assert_eq!(audit.len(), 1, "exactly one cancel attempt was made");
    let row = &audit[0];
    assert_eq!(row.get("source").and_then(|v| v.as_str()), Some("tui"));
    assert_eq!(
        row.get("host").and_then(|v| v.as_str()),
        Some("test-laptop")
    );
    assert_eq!(row.get("pid").and_then(|v| v.as_i64()), Some(4242));
    assert_eq!(
        row.get("reason").and_then(|v| v.as_str()),
        Some("user-pressed-cancel")
    );
    assert_eq!(
        row.get("in_flight_filename").and_then(|v| v.as_str()),
        Some("provenance.cha")
    );
    // First cancel against a non-terminal job is accepted.
    assert_eq!(row.get("accepted").and_then(|v| v.as_bool()), Some(true));
}

/// RED test (Phase 1, RED 1.2) — double-cancel produces two audit rows.
///
/// Mirrors the 2026-04-25 Malayalam incident on net, where a user pressed
/// cancel twice exactly an hour apart. Both presses must be visible in
/// the audit table (with `accepted=false` on the second when the job is
/// already terminal), so a forensic reader can see the duplicate-cancel
/// pattern that motivates the audit table in the first place.
#[tokio::test]
async fn cancel_twice_records_two_audit_rows() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "double-cancel.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tcancel .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id.clone();

    // First cancel — operator-pressed-cancel pattern.
    let body1 = serde_json::json!({
        "source": "tui",
        "host": "test-laptop",
        "pid": 1111,
        "reason": "user-pressed-cancel"
    });
    let resp = client
        .post(format!("{base_url}/jobs/{job_id}/cancel"))
        .json(&body1)
        .send()
        .await
        .expect("POST cancel #1");
    assert_eq!(resp.status(), 200, "first cancel should succeed");

    // Second cancel — same job, different PID (simulating a re-press).
    let body2 = serde_json::json!({
        "source": "tui",
        "host": "test-laptop",
        "pid": 2222,
        "reason": "user-pressed-cancel-again-nothing-happened"
    });
    let resp = client
        .post(format!("{base_url}/jobs/{job_id}/cancel"))
        .json(&body2)
        .send()
        .await
        .expect("POST cancel #2");
    assert_eq!(
        resp.status(),
        200,
        "second cancel against already-cancelled job should still 200"
    );

    // Audit table should have two rows: first accepted=true, second accepted=false.
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/cancellations"))
        .send()
        .await
        .expect("GET cancellations");
    assert_eq!(resp.status(), 200);
    let audit: Vec<serde_json::Value> = resp.json().await.expect("parse audit list");
    assert_eq!(
        audit.len(),
        2,
        "both cancel attempts must be persisted; got {audit:?}"
    );

    // Oldest first ordering preserves the temporal sequence.
    assert_eq!(audit[0].get("pid").and_then(|v| v.as_i64()), Some(1111));
    assert_eq!(audit[1].get("pid").and_then(|v| v.as_i64()), Some(2222));

    // First cancel mutated state, second was a no-op.
    assert_eq!(
        audit[0].get("accepted").and_then(|v| v.as_bool()),
        Some(true),
        "first cancel: state-changing"
    );
    assert_eq!(
        audit[1].get("accepted").and_then(|v| v.as_bool()),
        Some(false),
        "second cancel: job was already terminal"
    );

    // The denormalized columns reflect the MOST RECENT cancel (second).
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}"))
        .send()
        .await
        .expect("GET job");
    let raw_job: serde_json::Value = resp.json().await.expect("parse");
    assert_eq!(
        raw_job
            .get("last_cancelled_reason")
            .and_then(|v| v.as_str()),
        Some("user-pressed-cancel-again-nothing-happened"),
        "jobs.last_cancelled_reason must reflect the most recent cancel"
    );
}

/// `GET /jobs/{id}/cancellations` returns parseable
/// `CancellationRecord` rows that the CLI subcommand renders.
/// Locks down the wire contract end-to-end: typed POST body in,
/// audit row out, deserializes into the same type the CLI uses.
#[tokio::test]
async fn cancellations_endpoint_returns_typed_records() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();
    let submission = test_submission(vec![FilePayload {
        filename: "endpoint.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tcancel .\n@End\n".into(),
    }]);
    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id.clone();

    let body = serde_json::json!({
        "source": "tui",
        "host": "endpoint-test-host",
        "pid": 7777,
        "reason": "endpoint-typed-roundtrip",
        "in_flight_filename": "endpoint.cha"
    });
    let resp = client
        .post(format!("{base_url}/jobs/{job_id}/cancel"))
        .json(&body)
        .send()
        .await
        .expect("POST cancel");
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/cancellations"))
        .send()
        .await
        .expect("GET cancellations");
    assert_eq!(resp.status(), 200);
    let records: Vec<batchalign::api::CancellationRecord> = resp
        .json()
        .await
        .expect("parse typed CancellationRecord list");
    assert_eq!(records.len(), 1);
    let r = &records[0];
    assert_eq!(r.source, batchalign::api::CancelSource::Tui);
    assert_eq!(
        r.host.as_ref().map(|h| h.as_ref()),
        Some("endpoint-test-host")
    );
    assert_eq!(r.pid.map(|p| p.0), Some(7777));
    assert!(r.accepted);
}

#[tokio::test]
async fn delete_completed_job() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "delete.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tdelete .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    // Wait for completion
    poll_job_done(&client, &base_url, &job_id).await;

    // Delete
    let resp = client
        .delete(format!("{base_url}/jobs/{job_id}"))
        .send()
        .await
        .expect("DELETE");
    assert_eq!(resp.status(), 200);

    // Verify 404
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}"))
        .send()
        .await
        .expect("GET deleted");
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn unknown_command_returns_422() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Send raw JSON with an invalid command string — ReleasedCommand is a
    // closed enum, so axum's JSON extractor rejects unknown variants at
    // deserialization time (HTTP 422).
    let raw = serde_json::json!({
        "command": "nonexistent_command",
        "lang": "eng",
        "num_speakers": 1,
        "files": [{"filename": "bad.cha", "content": "content"}],
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
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn no_files_returns_400() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();
    let submission = test_submission(vec![]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn delete_running_job_returns_409() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Submit multiple files to increase the chance it's still running
    let files: Vec<FilePayload> = (0..5)
        .map(|i| FilePayload {
            filename: format!("file_{i}.cha").into(),
            content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
        })
        .collect();
    let submission = test_submission(files);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200, "POST /jobs should succeed");
    let body = resp.text().await.expect("read body");
    let info: JobInfo =
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("parse POST body: {e}\n{body}"));
    let job_id = info.job_id;

    // Try to delete immediately — might be running
    let resp = client
        .delete(format!("{base_url}/jobs/{job_id}"))
        .send()
        .await
        .expect("DELETE");

    let status = resp.status().as_u16();
    // It's either 409 (running) or 200 (already done, test-echo is fast)
    assert!(
        status == 409 || status == 200,
        "Expected 409 or 200, got {status}"
    );
}

#[tokio::test]
async fn job_not_found_returns_404() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let resp = reqwest::get(format!("{base_url}/jobs/nonexistent"))
        .await
        .expect("GET /jobs/nonexistent");
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn results_before_completion_returns_409() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Submit many files so it won't finish instantly
    let files: Vec<FilePayload> = (0..10)
        .map(|i| FilePayload {
            filename: format!("slow_{i}.cha").into(),
            content: "@UTF8\n@Begin\n*CHI:\thello .\n@End\n".into(),
        })
        .collect();
    let submission = test_submission(files);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200, "POST /jobs should succeed");
    let body = resp.text().await.expect("read body");
    let info: JobInfo =
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("parse POST body: {e}\n{body}"));
    let job_id = info.job_id;

    // Immediately request results — should be 409 or 200 (race)
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/results"))
        .send()
        .await
        .expect("GET results");

    let status = resp.status().as_u16();
    // Accept either 409 (still running) or 200 (already done — test-echo is fast)
    assert!(
        status == 409 || status == 200,
        "Expected 409 or 200, got {status}"
    );
}

#[tokio::test]
async fn restart_failed_job() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "restart.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\trestart .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    // Wait for completion
    poll_job_done(&client, &base_url, &job_id).await;

    // Cancel it first (so it's in a restartable state)
    // Actually, completed jobs can't be restarted — only cancelled/failed.
    // So let's submit and cancel quickly.
    let submission2 = test_submission(vec![FilePayload {
        filename: "restart2.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\trestart2 .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission2)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id2 = info.job_id;

    // Cancel it
    client
        .post(format!("{base_url}/jobs/{job_id2}/cancel"))
        .send()
        .await
        .expect("cancel");

    // Brief pause to let cancel propagate
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check status - if it's cancelled, try restart
    let resp = client
        .get(format!("{base_url}/jobs/{job_id2}"))
        .send()
        .await
        .expect("GET job");
    let info: JobInfo = resp.json().await.expect("parse");

    if info.status == JobStatus::Cancelled {
        let resp = client
            .post(format!("{base_url}/jobs/{job_id2}/restart"))
            .send()
            .await
            .expect("restart");
        assert_eq!(resp.status(), 200);

        let restarted: JobInfo = resp.json().await.expect("parse restart");
        assert_eq!(restarted.status, JobStatus::Queued);

        // Wait for re-completion
        let final_info = poll_job_done(&client, &base_url, &job_id2).await;
        assert_eq!(final_info.status, JobStatus::Completed);
    }
    // If it completed before we could cancel, that's fine — test-echo is fast
}

#[tokio::test]
async fn restart_completed_job_returns_409() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let submission = test_submission(vec![FilePayload {
        filename: "no_restart.cha".into(),
        content: "@UTF8\n@Begin\n*CHI:\tno_restart .\n@End\n".into(),
    }]);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    let info: JobInfo = resp.json().await.expect("parse");
    let job_id = info.job_id;

    poll_job_done(&client, &base_url, &job_id).await;

    // Try to restart a completed job — should be 409
    let resp = client
        .post(format!("{base_url}/jobs/{job_id}/restart"))
        .send()
        .await
        .expect("restart");
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn paths_mode_job() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Create input and output files for paths mode in the session's
    // state directory; the fixture cleans it up on session drop.
    let input_dir = session.state_dir().join("input");
    let output_dir = session.state_dir().join("output");
    std::fs::create_dir_all(&input_dir).expect("mkdir input");
    std::fs::create_dir_all(&output_dir).expect("mkdir output");

    let input_path = input_dir.join("paths_test.cha");
    let requested_output_path = output_dir.join("requested-output.cha");
    let written_output_path = output_dir.join("paths_test.cha");
    let input_content = "@UTF8\n@Begin\n*CHI:\tpaths .\n@End\n";
    std::fs::write(&input_path, input_content).expect("write input");

    let submission = JobSubmission {
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
        paths_mode: true,
        source_paths: vec![input_path.to_string_lossy().as_ref().into()],
        output_paths: vec![requested_output_path.to_string_lossy().as_ref().into()],
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
    assert_eq!(resp.status(), 200);

    let info: JobInfo = resp.json().await.expect("parse");
    assert_eq!(info.total_files, 1);
    let job_id = info.job_id;

    // Wait for completion
    let final_info = poll_job_done(&client, &base_url, &job_id).await;
    assert_eq!(final_info.status, JobStatus::Completed);
    assert_eq!(
        final_info
            .control_plane
            .as_ref()
            .map(|control| control.backend),
        Some(JobControlPlaneBackendKind::Test)
    );
    assert!(
        !requested_output_path.exists(),
        "paths_mode should derive the output filename from the source file"
    );
    let written = std::fs::read_to_string(&written_output_path)
        .expect("paths_mode should write the derived output file");
    assert_eq!(written, input_content);
}

#[tokio::test]
async fn multiple_files_in_one_job() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    let files: Vec<FilePayload> = (0..3)
        .map(|i| FilePayload {
            filename: format!("multi_{i}.cha").into(),
            content: format!("@UTF8\n@Begin\n*CHI:\tfile{i} .\n@End\n"),
        })
        .collect();

    let submission = test_submission(files);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200);

    let info: JobInfo = resp.json().await.expect("parse");
    assert_eq!(info.total_files, 3);
    let job_id = info.job_id;

    let final_info = poll_job_done(&client, &base_url, &job_id).await;
    assert_eq!(final_info.status, JobStatus::Completed);
    assert_eq!(final_info.completed_files, 3);
}

#[tokio::test]
async fn multi_file_job_uses_parallel_workers() {
    let config = ServerConfig {
        host: "127.0.0.1".into(),
        port: 0,
        warmup_commands: vec![],
        max_workers_per_job: Some(3), // Force 3 parallel workers
        memory_gate_mb: Some(MemoryMb(0)),
        ..Default::default()
    };
    let Some(session) = acquire_test_server_session_with_config(config).await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Submit 5 files in one job
    let files: Vec<FilePayload> = (0..5)
        .map(|i| FilePayload {
            filename: format!("parallel_{i}.cha").into(),
            content: format!("@UTF8\n@Begin\n*CHI:\tparallel{i} .\n@End\n"),
        })
        .collect();

    let submission = test_submission(files);

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200);

    let info: JobInfo = resp.json().await.expect("parse");
    assert_eq!(info.total_files, 5);
    let job_id = info.job_id;

    // Wait for completion
    let final_info = poll_job_done(&client, &base_url, &job_id).await;
    assert_eq!(final_info.status, JobStatus::Completed);
    assert_eq!(final_info.completed_files, 5);

    // Verify num_workers was set (should be min(3, 5) = 3)
    assert!(
        final_info.num_workers.is_some(),
        "Expected num_workers to be set"
    );
    let nw = final_info.num_workers.unwrap();
    assert!(
        (1..=3).contains(&nw),
        "Expected num_workers in [1, 3], got {nw}"
    );

    // Verify all results are accessible
    let resp = client
        .get(format!("{base_url}/jobs/{job_id}/results"))
        .send()
        .await
        .expect("GET results");
    assert_eq!(resp.status(), 200);

    let results: JobResultResponse = resp.json().await.expect("parse results");
    assert_eq!(results.files.len(), 5);
    for file in &results.files {
        assert!(file.error.is_none(), "unexpected error: {:?}", file.error);
    }
}

// ---------------------------------------------------------------------------
// Capability gate: real worker (not test-echo)
// ---------------------------------------------------------------------------

/// Verify that create_test_app succeeds with a real Python worker whose import
/// probes (commands) are broader than its loaded infer tasks. Before the fix,
/// this would crash with "worker capability gate failed".
#[tokio::test]
async fn server_starts_with_real_worker_capability_gate() {
    let python_path = require_python!();
    // This test bypasses the shared fixture (test_echo=false), so opt
    // into the same per-process ledger override the fixture uses.
    isolate_host_memory_ledger();

    let config = ServerConfig {
        host: "127.0.0.1".into(),
        port: 0,
        job_ttl_days: 7,
        warmup_commands: vec![],
        memory_gate_mb: Some(MemoryMb(0)),
        ..Default::default()
    };

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let jobs_dir = tmp.path().join("jobs");
    std::fs::create_dir_all(&jobs_dir).expect("mkdir jobs");
    let db_dir = tmp.path().join("db");
    std::fs::create_dir_all(&db_dir).expect("mkdir db");

    // Real worker, NOT test-echo. This exercises the capability gate.
    let pool_config = PoolConfig {
        python_path: python_path.clone(),
        test_echo: false,
        health_check_interval_s: 600,
        ready_timeout_s: 30,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    };

    let result = create_test_app(
        config,
        pool_config,
        Some(jobs_dir.to_string_lossy().into()),
        Some(db_dir),
        Some("test-build-hash".into()),
    )
    .await;

    match result {
        Ok((router, state)) => {
            // Server started — verify capabilities were filtered, not rejected.
            assert!(
                !state.capabilities().is_empty(),
                "should have at least one capability"
            );
            eprintln!(
                "Server started OK. Capabilities: {:?}, Infer tasks: {:?}",
                state.capabilities(),
                state.infer_tasks()
            );

            // Quick health check
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let port = listener.local_addr().expect("local_addr").port();
            tokio::spawn(async move {
                axum::serve(
                    listener,
                    router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await
                .ok();
            });
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let client = reqwest::Client::new();
            let resp = client
                .get(format!("http://127.0.0.1:{port}/health"))
                .send()
                .await
                .expect("health request");
            assert!(resp.status().is_success());
            let health: HealthResponse = resp.json().await.expect("parse health");
            assert_eq!(health.status, HealthStatus::Ok);
        }
        Err(e) => {
            panic!(
                "create_test_app should succeed with real worker after capability gate fix, \
                 but failed: {e}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: poll until job reaches terminal state
// ---------------------------------------------------------------------------

async fn poll_job_done(client: &reqwest::Client, base_url: &str, job_id: &str) -> JobInfo {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(60);
    let mut poll_count = 0u32;

    loop {
        let resp = client
            .get(format!("{base_url}/jobs/{job_id}"))
            .send()
            .await
            .expect("GET job");
        let status_code = resp.status();
        let body = resp.text().await.expect("read body");
        let info: JobInfo = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("parse job failed (HTTP {status_code}): {e}\nbody: {body}"));

        poll_count += 1;
        if poll_count <= 3 || poll_count.is_multiple_of(50) {
            eprintln!(
                "  poll #{poll_count}: job={job_id} status={:?} completed={}/{}",
                info.status, info.completed_files, info.total_files
            );
        }

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

// ---------------------------------------------------------------------------
// Concurrent jobs
// ---------------------------------------------------------------------------

/// Submit 3 jobs simultaneously, all should complete successfully.
#[tokio::test]
async fn concurrent_jobs_complete() {
    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let base_url = session.base_url();

    let client = reqwest::Client::new();

    // Submit 3 jobs concurrently
    let mut job_ids = Vec::new();
    for i in 0..3 {
        let sub = test_submission(vec![FilePayload {
            filename: format!("concurrent_{i}.cha").into(),
            content: format!("@UTF8\n@Begin\n*CHI:\tconcurrent{i} .\n@End\n"),
        }]);

        let resp = client
            .post(format!("{base_url}/jobs"))
            .json(&sub)
            .send()
            .await
            .expect("POST /jobs");
        assert_eq!(resp.status(), 200);
        let info: JobInfo = resp.json().await.expect("parse");
        job_ids.push(info.job_id.clone());
    }

    // Poll all 3 concurrently
    let (r1, r2, r3) = tokio::join!(
        poll_job_done(&client, &base_url, &job_ids[0]),
        poll_job_done(&client, &base_url, &job_ids[1]),
        poll_job_done(&client, &base_url, &job_ids[2]),
    );

    assert_eq!(r1.status, JobStatus::Completed, "Job 0 should complete");
    assert_eq!(r2.status, JobStatus::Completed, "Job 1 should complete");
    assert_eq!(r3.status, JobStatus::Completed, "Job 2 should complete");

    // Verify results for each job
    for (i, job_id) in job_ids.iter().enumerate() {
        let resp = client
            .get(format!("{base_url}/jobs/{job_id}/results"))
            .send()
            .await
            .expect("GET results");
        assert_eq!(resp.status(), 200);
        let results: JobResultResponse = resp.json().await.expect("parse results");
        assert_eq!(results.files.len(), 1, "Job {i} should have 1 result");
        assert!(
            results.files[0].error.is_none(),
            "Job {i} should have no error"
        );
    }
}
