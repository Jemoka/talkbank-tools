// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! JSON compatibility tests — verify Rust serde output matches Python Pydantic.
//!
//! These tests use insta snapshots so that if the JSON format changes, we get
//! a clear diff. The snapshots represent the canonical wire format that both
//! Rust and Python must agree on.

use batchalign::api::*;
use batchalign::api::{LanguageCode3, LanguageSpec, WorkerLanguage};
use batchalign::config::ServerConfig;
use batchalign::host_memory::HostMemoryPressureLevel;
use batchalign::options::{AlignOptions, CommandOptions, CommonOptions, MorphotagOptions};
use std::collections::BTreeMap;

use batchalign::worker::{
    BatchInferRequest, BatchInferResponse, InferRequest, InferResponse, InferTask,
    WorkerCapabilities, WorkerHealthResponse, WorkerHealthStatus, WorkerPid,
};

// ---------------------------------------------------------------------------
// API types
// ---------------------------------------------------------------------------

#[test]
fn snapshot_job_submission_minimal() {
    let sub = JobSubmission {
        command: ReleasedCommand::Morphotag,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options: CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        }),
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };
    insta::assert_json_snapshot!("job_submission_minimal", sub);
}

#[test]
fn snapshot_job_submission_with_files() {
    let sub = JobSubmission {
        command: ReleasedCommand::Align,
        lang: LanguageSpec::Resolved(LanguageCode3::spa()),
        num_speakers: NumSpeakers(2),
        files: vec![FilePayload {
            filename: "01DM_18.cha".into(),
            content: "@UTF8\n@Begin\n*CHI:\thello .\n@End".into(),
        }],
        media_files: vec![],
        media_mapping: "childes-data".into(),
        media_subdir: "Eng-NA/MacWhinney".into(),
        source_dir: "/data/corpus".into(),
        options: CommandOptions::Align(AlignOptions {
            common: CommonOptions::default(),
            fa_engine: batchalign::options::FaEngineName::Wave2Vec,
            utr_engine: None,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: true,
            wor: true.into(),
            merge_abbrev: false.into(),
            bullet_repair: false,
            review_level: Default::default(),
            media_dir: None,
        }),
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };
    insta::assert_json_snapshot!("job_submission_with_files", sub);
}

#[test]
fn snapshot_job_submission_paths_mode() {
    let sub = JobSubmission {
        command: ReleasedCommand::Morphotag,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: "/input".into(),
        options: CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        }),
        paths_mode: true,
        source_paths: vec!["/input/a.cha".into(), "/input/b.cha".into()],
        output_paths: vec!["/output/a.cha".into(), "/output/b.cha".into()],
        display_names: vec!["a.cha".into(), "b.cha".into()],
        debug_traces: false,
        before_paths: vec![],
    };
    insta::assert_json_snapshot!("job_submission_paths_mode", sub);
}

#[test]
fn snapshot_job_status_all_variants() {
    let statuses = vec![
        JobStatus::Queued,
        JobStatus::Running,
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Cancelled,
        JobStatus::Interrupted,
    ];
    insta::assert_json_snapshot!("job_status_all_variants", statuses);
}

#[test]
fn snapshot_job_info() {
    let info = JobInfo {
        job_id: "550e8400-e29b-41d4-a716-446655440000".into(),
        status: JobStatus::Running,
        command: ReleasedCommand::Morphotag,
        options: batchalign::options::CommandOptions::Morphotag(
            batchalign::options::MorphotagOptions {
                common: batchalign::options::CommonOptions::default(),

                ..Default::default()
            },
        ),
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        source_dir: "/data/corpus".into(),
        total_files: 10,
        completed_files: 3,
        current_file: Some("04DM.cha".into()),
        error: None,
        file_statuses: vec![
            FileStatusEntry {
                filename: "01DM.cha".into(),
                status: FileStatusKind::Done,
                error: None,
                error_category: None,
                error_codes: None,
                error_line: None,
                bug_report_id: None,
                started_at: Some(UnixTimestamp(1700000000.0)),
                finished_at: Some(UnixTimestamp(1700000005.0)),
                next_eligible_at: None,
                progress_current: None,
                progress_total: None,
                progress_stage: None,
                progress_label: None,
            },
            FileStatusEntry {
                filename: "04DM.cha".into(),
                status: FileStatusKind::Processing,
                error: None,
                error_category: None,
                error_codes: None,
                error_line: None,
                bug_report_id: None,
                started_at: Some(UnixTimestamp(1700000005.0)),
                finished_at: None,
                next_eligible_at: None,
                progress_current: Some(2),
                progress_total: Some(5),
                progress_stage: Some(FileProgressStage::AnalyzingMorphosyntax),
                progress_label: Some("Analyzing morphosyntax".into()),
            },
        ],
        submitted_at: Some("2026-01-15T10:00:00Z".into()),
        submitted_by: Some("192.168.1.1".into()),
        submitted_by_name: Some("Lab-Mac-1".into()),
        completed_at: None,
        duration_s: None,
        next_eligible_at: None,
        num_workers: Some(4),
        active_lease: None,
        control_plane: None,
        batch_progress: None,
        execution_plan: None,
        last_cancelled_at: None,
        last_cancelled_source: None,
        last_cancelled_host: None,
        last_cancelled_reason: None,
    };
    insta::assert_json_snapshot!("job_info", info);
}

#[test]
fn snapshot_job_result_response() {
    let resp = JobResultResponse {
        job_id: "abc-123".into(),
        status: JobStatus::Completed,
        files: vec![
            FileResult {
                filename: "01DM.cha".into(),
                content: "@UTF8\n@Begin\n*CHI:\thello .\n@End".into(),
                content_type: ContentType::Chat,
                error: None,
                provenance: Vec::new(),
            },
            FileResult {
                filename: "02DM.cha".into(),
                content: String::new(),
                content_type: ContentType::Chat,
                error: Some("Pipeline error: unknown language".into()),
                provenance: Vec::new(),
            },
        ],
    };
    insta::assert_json_snapshot!("job_result_response", resp);
}

#[test]
fn snapshot_job_list_item() {
    let item = JobListItem {
        job_id: "abc-123".into(),
        status: JobStatus::Completed,
        command: ReleasedCommand::Morphotag,
        lang: LanguageSpec::Resolved(LanguageCode3::eng()),
        source_dir: "/data/corpus".into(),
        total_files: 10,
        completed_files: 10,
        error_files: 1,
        submitted_at: Some("2026-01-15T10:00:00Z".into()),
        submitted_by: Some("192.168.1.1".into()),
        submitted_by_name: Some("Lab-Mac-1".into()),
        completed_at: Some("2026-01-15T10:05:00Z".into()),
        duration_s: Some(DurationSeconds(300.0)),
        next_eligible_at: None,
        num_workers: Some(4),
        active_lease: None,
        control_plane: None,
    };
    insta::assert_json_snapshot!("job_list_item", item);
}

#[test]
fn snapshot_health_response() {
    let health = HealthResponse {
        status: HealthStatus::Ok,
        version: "0.10.0".into(),
        free_threaded: true,
        capabilities: vec!["align".into(), "morphotag".into(), "transcribe".into()],
        loaded_pipelines: vec!["morphotag:eng:1".into()],
        media_roots: vec!["/data/media".into()],
        media_mapping_keys: vec!["childes-data".into()],
        workers_available: 3,
        job_slots_available: 3,
        live_workers: 1,
        live_worker_keys: vec!["morphotag:eng".into()],
        active_jobs: 1,
        cache_backend: "sqlite".into(),
        worker_crashes: 0,
        attempts_started: 0,
        attempts_retried: 0,
        deferred_work_units: 0,
        forced_terminal_errors: 0,
        memory_gate_aborts: 0,
        build_hash: "0.10.0-abc1234-1700000000".into(),
        node_id: NodeId::default(),
        warmup_status: batchalign::worker::pool::WarmupStatus::Complete,
        system_memory_total_mb: MemoryMb(65536),
        system_memory_available_mb: MemoryMb(32768),
        system_memory_used_mb: MemoryMb(32768),
        memory_gate_threshold_mb: MemoryMb(8192),
        host_memory_pressure: HostMemoryPressureLevel::Guarded,
        host_memory_reserved_mb: MemoryMb(12000),
        host_memory_startup_leases: 1,
        host_memory_job_leases: 1,
        host_memory_ml_test_locks: 0,
        host_memory_active_leases: vec![
            "WorkerStartup:worker-startup:profile:gpu:eng:{}:16000MB".into(),
            "JobExecution:job-execution:job-1:align:eng:12000MB".into(),
        ],
        host_memory_error: None,
    };
    insta::assert_json_snapshot!("health_response", health);
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[test]
fn snapshot_server_config_default() {
    // The default task queue is derived from `sysinfo::System::host_name()`,
    // which makes the value host-dependent and also leaks a fleet hostname
    // into a snapshot that is checked into a public repo. Override to a
    // stable placeholder so the snapshot is deterministic across machines.
    let cfg = ServerConfig {
        temporal_task_queue: batchalign::api::TemporalTaskQueue::from(
            "batchalign3-testhost".to_string(),
        ),
        ..Default::default()
    };
    insta::assert_json_snapshot!("server_config_default", cfg);
}

#[test]
fn snapshot_server_config_full() {
    use batchalign_types::paths::{MediaMappingKey, ServerPath};
    let mut mappings = std::collections::BTreeMap::new();
    mappings.insert(
        MediaMappingKey::new("childes-data"),
        ServerPath::new("/nfs/childes"),
    );
    mappings.insert(
        MediaMappingKey::new("aphasia-data"),
        ServerPath::new("/nfs/aphasia"),
    );

    let cfg = ServerConfig {
        media_roots: vec![
            ServerPath::new("/data/media"),
            ServerPath::new("/data/media2"),
        ],
        media_mappings: mappings,
        default_lang: LanguageCode3::spa(),
        max_concurrent_jobs: Some(4),
        port: 9000,
        host: "0.0.0.0".into(),
        max_workers_per_job: Some(2),
        job_ttl_days: 14,
        warmup_commands: vec!["morphotag".into(), "align".into()],
        auto_daemon: true,
        memory_gate_mb: Some(MemoryMb(2048)),
        worker_health_interval_s: 15,
        temporal_task_queue: batchalign::api::TemporalTaskQueue::from(
            "batchalign3-testhost".to_string(),
        ),
        ..Default::default()
    };
    insta::assert_json_snapshot!("server_config_full", cfg);
}

// ---------------------------------------------------------------------------
// Worker IPC
// ---------------------------------------------------------------------------

#[test]
fn snapshot_worker_health() {
    let health = WorkerHealthResponse {
        status: WorkerHealthStatus::Ok,
        command: "infer:morphosyntax".into(),
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        pid: WorkerPid(12345),
        uptime_s: DurationSeconds(120.5),
    };
    insta::assert_json_snapshot!("worker_health", health);
}

#[test]
fn snapshot_worker_capabilities() {
    let caps = WorkerCapabilities {
        commands: vec!["align".into(), "morphotag".into(), "transcribe".into()],
        free_threaded: true,
        infer_tasks: vec![InferTask::Morphosyntax, InferTask::Utseg],
        engine_versions: std::collections::BTreeMap::from([
            ("morphosyntax".into(), "stanza-1.9.2".into()),
            ("utseg".into(), "stanza-1.9.2".into()),
        ]),
        stanza_capabilities: Default::default(),
    };
    insta::assert_json_snapshot!("worker_capabilities", caps);
}

// ---------------------------------------------------------------------------
// Cross-language deserialization: verify Rust can parse Python-style JSON
// ---------------------------------------------------------------------------

/// Simulate what Python `JobSubmission.model_dump_json()` produces.
#[test]
fn deserialize_python_job_submission() {
    let python_json = r#"{
        "command": "morphotag",
        "lang": "eng",
        "num_speakers": 1,
        "files": [
            {"filename": "test.cha", "content": "@UTF8\n@Begin\n@End"}
        ],
        "media_files": [],
        "media_mapping": "",
        "media_subdir": "",
        "source_dir": "",
        "options": {"command": "morphotag", "retokenize": false},
        "paths_mode": false,
        "source_paths": [],
        "output_paths": [],
        "display_names": []
    }"#;

    let sub: JobSubmission = serde_json::from_str(python_json).unwrap();
    assert_eq!(sub.command, ReleasedCommand::Morphotag);
    assert_eq!(sub.files.len(), 1);
    assert_eq!(sub.files[0].filename, "test.cha");
    assert!(!sub.paths_mode);
}

/// Simulate Python `HealthResponse.model_dump_json()` with all fields.
#[test]
fn deserialize_python_health_response() {
    let python_json = r#"{
        "status": "ok",
        "version": "0.10.0",
        "free_threaded": true,
        "capabilities": ["align", "morphotag"],
        "loaded_pipelines": ["morphotag:eng:1"],
        "media_roots": ["/data/media"],
        "media_mapping_keys": ["childes-data"],
        "workers_available": 3,
        "job_slots_available": 3,
        "live_workers": 1,
        "live_worker_keys": ["morphotag:eng"],
        "active_jobs": 1,
        "cache_backend": "sqlite",
        "worker_crashes": 0,
        "forced_terminal_errors": 0,
        "memory_gate_aborts": 0
    }"#;

    let health: HealthResponse = serde_json::from_str(python_json).unwrap();
    assert_eq!(health.status, HealthStatus::Ok);
    assert!(health.free_threaded);
    assert_eq!(health.capabilities, vec!["align", "morphotag"]);
}

/// Verify ServerConfig deserializes from YAML that Python OmegaConf produces.
#[test]
fn deserialize_python_server_config_yaml() {
    let yaml = r#"
media_roots:
  - /Volumes/Media/talkbank
media_mappings:
  childes-data: /Volumes/Media/childes
  aphasia-data: /Volumes/Media/aphasia
default_lang: eng
max_concurrent_jobs: 0
port: 8000
host: "0.0.0.0"
max_workers_per_job: 0
job_ttl_days: 7
warmup_commands: []
auto_daemon: false
"#;

    use batchalign_types::paths::{MediaMappingKey, ServerPath};
    let cfg: ServerConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        cfg.media_roots,
        vec![ServerPath::new("/Volumes/Media/talkbank")]
    );
    assert_eq!(
        cfg.media_mappings[&MediaMappingKey::new("childes-data")],
        ServerPath::new("/Volumes/Media/childes")
    );
    assert_eq!(cfg.port, 8000);
    assert!(cfg.warmup_commands.is_empty());
}

/// Verify InferRequest/InferResponse roundtrip matches Python worker.py.
#[test]
fn infer_ipc_roundtrip() {
    // Simulate what Rust sends to a Python worker
    let req = InferRequest {
        task: InferTask::Morphosyntax,
        lang: LanguageCode3::eng(),
        payload: serde_json::json!({
            "words": ["the", "dog", "runs"],
            "terminator": ".",
            "special_forms": []
        }),
    };

    let req_json = serde_json::to_string(&req).unwrap();
    let req_back: InferRequest = serde_json::from_str(&req_json).unwrap();
    assert_eq!(req.task, req_back.task);
    assert_eq!(req.lang, req_back.lang);
    assert_eq!(req.payload, req_back.payload);

    // Simulate what Python worker responds with
    let resp_json = r#"{
        "result": {"mor": "det|the n|dog v|run-3S", "gra": "1|2|DET 2|3|SUBJ 3|0|ROOT"},
        "error": null,
        "elapsed_s": 0.123
    }"#;

    let resp: InferResponse = serde_json::from_str(resp_json).unwrap();
    assert!(resp.result.is_some());
    assert!(resp.error.is_none());
    assert_eq!(resp.elapsed_s, 0.123);
}

/// Verify BatchInferRequest/BatchInferResponse roundtrip.
#[test]
fn batch_infer_ipc_roundtrip() {
    let req = BatchInferRequest {
        task: InferTask::Morphosyntax,
        lang: LanguageCode3::eng(),
        items: vec![
            serde_json::json!({"words": ["hello"], "terminator": "."}),
            serde_json::json!({"words": ["world"], "terminator": "."}),
        ],
        mwt: BTreeMap::new(),
        allow_stanza_fallback: false,
    };

    let req_json = serde_json::to_string(&req).unwrap();
    let req_back: BatchInferRequest = serde_json::from_str(&req_json).unwrap();
    assert_eq!(req.task, req_back.task);
    assert_eq!(req.items.len(), 2);

    // Simulate Python response
    let resp_json = r#"{
        "results": [
            {"result": {"mor": "n|hello"}, "error": null, "elapsed_s": 0.05},
            {"result": null, "error": "failed for testing", "elapsed_s": 0.0}
        ]
    }"#;

    let resp: BatchInferResponse = serde_json::from_str(resp_json).unwrap();
    assert_eq!(resp.results.len(), 2);
    assert!(resp.results[0].result.is_some());
    assert!(resp.results[0].error.is_none());
    assert!(resp.results[1].result.is_none());
    assert_eq!(resp.results[1].error.as_deref(), Some("failed for testing"));
}

/// Verify Python CapabilitiesResponse with infer fields parses in Rust.
#[test]
fn deserialize_python_capabilities_with_infer() {
    let python_json = r#"{
        "commands": ["morphotag", "align"],
        "free_threaded": false,
        "infer_tasks": ["morphosyntax", "utseg"],
        "engine_versions": {"morphosyntax": "stanza-1.9.2", "utseg": "stanza-1.9.2"}
    }"#;

    let caps: WorkerCapabilities = serde_json::from_str(python_json).unwrap();
    assert_eq!(
        caps.infer_tasks,
        vec![InferTask::Morphosyntax, InferTask::Utseg]
    );
    assert_eq!(caps.engine_versions["morphosyntax"], "stanza-1.9.2");
}

/// Missing infer capability fields should fail deserialization.
#[test]
fn deserialize_python_capabilities_without_infer_is_rejected() {
    let python_json = r#"{
        "commands": ["morphotag"],
        "free_threaded": false
    }"#;

    let err = serde_json::from_str::<WorkerCapabilities>(python_json).unwrap_err();
    assert!(err.to_string().contains("infer_tasks"));
}
