// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Integration tests for the infer-era Python worker.
//!
//! These tests spawn real Python workers in `--test-echo` mode
//! (no ML models) and verify the Rust side can communicate over the infer-only
//! worker protocol.
//!
//! Requirements: Python 3 with batchalign installed.
//! Skip gracefully if unavailable.

mod common;

use batchalign::api::{LanguageCode3, NumSpeakers, ReleasedCommand, WorkerLanguage};
use batchalign::host_facts::PerProfile;
use batchalign::worker::error::WorkerError;
use batchalign::worker::handle::{
    WorkerConfig, WorkerHandle, WorkerRuntimeConfig, spawn_tcp_daemon,
};
use batchalign::worker::pool::{PoolConfig, WorkerPool};
use batchalign::worker::registry::{RegistryOwnership, read_registry};
use batchalign::worker::{
    BatchInferRequest, InferRequest, InferTask, WorkerBootstrapMode, WorkerProfile,
};
use common::resolve_python;
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

macro_rules! require_python {
    () => {{
        // Tests in this binary spawn workers that touch the host-memory
        // ledger via the spawn memory guard. Isolate the ledger to a
        // per-process file so we don't race against other test binaries.
        common::test_server_fixture::isolate_host_memory_ledger();
        // Memory guard: skip test entirely if insufficient RAM to safely spawn workers.
        // This prevents kernel OOM panics that have crashed contributor machines repeatedly.
        let available_mb = batchalign::worker::memory_guard::available_memory_mb();
        if available_mb < 4096 {
            eprintln!(
                "SKIP: insufficient memory ({available_mb} MB available, 4096 MB required). \
                 Worker tests need at least 4 GB free RAM."
            );
            return;
        }
        match resolve_python() {
            Some(path) => path,
            None => {
                eprintln!("SKIP: Python 3 with batchalign not available");
                return;
            }
        }
    }};
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => unsafe {
                std::env::set_var(self.key, previous);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) only checks process existence/permission.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(unix)]
fn terminate_pid(pid: u32) {
    // SAFETY: sending SIGTERM to an exact PID is the intended cleanup path.
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
}

#[cfg(unix)]
async fn wait_for_process_exit(pid: u32) {
    for _ in 0..50 {
        if !process_alive(pid) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("worker pid {pid} was still alive after waiting for exit");
}

#[cfg(unix)]
fn registry_path_for(state_dir: &tempfile::TempDir) -> std::path::PathBuf {
    state_dir.path().join("workers.json")
}

#[cfg(unix)]
fn test_echo_tcp_config(python_path: String) -> WorkerConfig {
    WorkerConfig {
        python_path,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    }
}

fn infer_request(payload: Value) -> InferRequest {
    InferRequest {
        task: InferTask::Morphosyntax,
        lang: LanguageCode3::eng(),
        payload,
    }
}

fn batch_request(task: InferTask, items: Vec<Value>) -> BatchInferRequest {
    BatchInferRequest {
        task,
        lang: LanguageCode3::eng(),
        items,
        mwt: BTreeMap::new(),
        allow_stanza_fallback: false,
    }
}

#[tokio::test]
async fn spawn_test_echo_worker() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");
    assert!(*lease.pid() > 0, "should have a valid pid");
    assert_eq!(lease.profile_label(), "profile:stanza");
    assert_eq!(lease.lang(), "eng");
    assert_eq!(lease.transport(), "stdio");
}

#[tokio::test]
async fn health_check_works() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");
    let health = lease.health_check().await.expect("health check failed");
    assert_eq!(health.status, batchalign::worker::WorkerHealthStatus::Ok);
    assert_eq!(health.command, "profile:stanza");
    assert_eq!(health.lang, WorkerLanguage::from(LanguageCode3::eng()));
    assert!(*health.pid > 0);
}

#[tokio::test]
async fn spawn_test_echo_worker_task_bootstrap() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        task: Some(InferTask::Morphosyntax),
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");
    assert_eq!(lease.profile_label(), "infer:morphosyntax");

    let health = lease.health_check().await.expect("health check failed");
    assert_eq!(health.command, "infer:morphosyntax");
}

#[tokio::test]
async fn capabilities_test_echo() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");
    let caps = lease.capabilities().await.expect("capabilities failed");
    assert!(caps.commands.iter().any(|c| c == "test-echo"));
    assert!(caps.commands.iter().any(|c| c == "morphotag"));
    // Test-echo workers intentionally advertise *all* infer tasks
    // and stamp every entry with engine_version `"test-echo"`. The
    // contract is documented in `python/batchalign/worker/_handlers.py`
    // (`_capabilities` test-echo branch): without that universal
    // advertisement the server's capability gate
    // (`AppState::validate_infer_capability_gate`) refuses to
    // dispatch jobs to an echo worker, which would block any
    // `test_echo: true` integration test that exercises the
    // dispatch path. Keep these assertions in sync with the Python
    // handler — when one side changes, the other must match.
    assert!(
        !caps.infer_tasks.is_empty(),
        "test-echo worker must advertise infer tasks so the capability gate passes"
    );
    assert!(
        caps.infer_tasks.contains(&InferTask::Morphosyntax),
        "test-echo capabilities must include Morphosyntax (smoke check on the universal-advertise contract)"
    );
    assert!(
        caps.engine_versions.values().all(|v| v == "test-echo"),
        "every engine_version on a test-echo worker must be the string \"test-echo\"; got {:?}",
        caps.engine_versions
    );
    assert!(!caps.free_threaded);
}

#[tokio::test]
async fn infer_echo_returns_payload() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");
    let payload = json!({"words": ["hello", "world"], "lang": "eng"});
    let response = lease
        .infer(&infer_request(payload.clone()))
        .await
        .expect("infer failed");
    assert_eq!(response.result, Some(payload));
    assert!(response.error.is_none());
}

#[tokio::test]
async fn batch_infer_echo_returns_items() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut lease = common::test_worker_pool::shared_test_worker_pool()
        .checkout(&config)
        .await
        .expect("checkout failed");
    let items = vec![
        json!({"words": ["hello"], "lang": "eng"}),
        json!({"words": ["world"], "lang": "eng"}),
    ];
    let response = lease
        .batch_infer(&batch_request(InferTask::Morphosyntax, items.clone()))
        .await
        .expect("batch infer failed");
    assert_eq!(response.results.len(), 2);
    assert_eq!(response.results[0].result, Some(items[0].clone()));
    assert_eq!(response.results[1].result, Some(items[1].clone()));
}

#[tokio::test]
async fn pool_dispatch_batch_infer_spawns_and_processes() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    let item = json!({"words": ["hello", "pool"], "lang": "eng"});
    let response = pool
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(InferTask::Morphosyntax, vec![item.clone()]),
        )
        .await
        .expect("dispatch failed");
    assert_eq!(response.results[0].result, Some(item));

    assert_eq!(pool.worker_count().await, 1);
    let summary = pool.worker_summary().await;
    assert_eq!(summary.len(), 1);
    assert!(summary[0].starts_with("profile:stanza:eng:pid="));
    assert!(summary[0].contains(":transport=stdio"));

    pool.shutdown().await;
    assert_eq!(pool.worker_count().await, 0);
}

#[tokio::test]
async fn pool_reuses_existing_worker() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    for i in 0..3 {
        let item = json!({"request": i});
        let response = pool
            .dispatch_batch_infer(
                &LanguageCode3::eng(),
                &batch_request(InferTask::Morphosyntax, vec![item.clone()]),
            )
            .await
            .expect("dispatch failed");
        assert_eq!(response.results[0].result, Some(item));
    }

    assert_eq!(pool.worker_count().await, 1);
    pool.shutdown().await;
}

#[tokio::test]
async fn pool_multiple_task_groups() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    let morph_item = json!({"task": "morph"});
    let fa_item = json!({"task": "fa"});
    let r1 = pool
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(InferTask::Morphosyntax, vec![morph_item.clone()]),
        )
        .await
        .expect("dispatch 1 failed");
    let r2 = pool
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(InferTask::Fa, vec![fa_item.clone()]),
        )
        .await
        .expect("dispatch 2 failed");
    assert_eq!(r1.results[0].result, Some(morph_item));
    assert_eq!(r2.results[0].result, Some(fa_item));
    assert_eq!(pool.worker_count().await, 2);

    pool.shutdown().await;
}

#[tokio::test]
async fn pool_task_bootstrap_separates_same_profile_tasks() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: WorkerRuntimeConfig::default().with_bootstrap_mode(WorkerBootstrapMode::Task),
        ..Default::default()
    });

    let morph_item = json!({"task": "morph"});
    let coref_item = json!({"task": "coref"});
    let r1 = pool
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(InferTask::Morphosyntax, vec![morph_item.clone()]),
        )
        .await
        .expect("dispatch 1 failed");
    let r2 = pool
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(InferTask::Coref, vec![coref_item.clone()]),
        )
        .await
        .expect("dispatch 2 failed");
    assert_eq!(r1.results[0].result, Some(morph_item));
    assert_eq!(r2.results[0].result, Some(coref_item));
    assert_eq!(pool.worker_count().await, 2);
    let summary = pool.worker_summary().await;
    assert!(
        summary
            .iter()
            .any(|entry| entry.starts_with("infer:morphosyntax:eng:")),
        "expected infer:morphosyntax worker in summary: {summary:?}"
    );
    assert!(
        summary
            .iter()
            .any(|entry| entry.starts_with("infer:coref:eng:")),
        "expected infer:coref worker in summary: {summary:?}"
    );

    pool.shutdown().await;
}

#[tokio::test]
async fn pool_warmup_uses_infer_targets() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    pool.warmup(&[
        batchalign::server::WarmupTarget {
            command: ReleasedCommand::Morphotag,
            lang: WorkerLanguage::from(LanguageCode3::eng()),
        },
        batchalign::server::WarmupTarget {
            command: ReleasedCommand::Align,
            lang: WorkerLanguage::from(LanguageCode3::eng()),
        },
    ])
    .await;

    let summary = pool.worker_summary().await;
    // morphotag → Stanza profile (sequential group), align → GPU profile (SharedGpuWorker)
    assert_eq!(pool.worker_count().await, 2);
    assert!(
        summary
            .iter()
            .any(|entry| entry.starts_with("profile:stanza:eng:")),
        "expected a Stanza profile worker in summary: {summary:?}"
    );
    assert!(
        summary
            .iter()
            .any(|entry| entry.starts_with("profile:gpu:eng:")),
        "expected a GPU profile worker in summary: {summary:?}"
    );

    pool.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn discover_from_registry_seeds_capabilities_from_external_tcp_daemon() {
    let python = require_python!();
    let state_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvVarGuard::set_path("BATCHALIGN_STATE_DIR", state_dir.path());
    let registry_path = registry_path_for(&state_dir);

    let external = test_echo_tcp_config(python.clone());
    let (external_pid, _external_port) = spawn_tcp_daemon(&external, 0)
        .await
        .expect("spawn external tcp daemon");

    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        worker_registry_path: registry_path.to_string_lossy().into_owned(),
        ..Default::default()
    });

    let discovered = pool.discover_from_registry().await;
    assert_eq!(discovered, 1, "expected one external registry worker");
    assert!(
        pool.detected_capabilities().is_some(),
        "registry discovery should seed a live capability snapshot"
    );

    pool.shutdown().await;
    if process_alive(external_pid) {
        terminate_pid(external_pid);
        wait_for_process_exit(external_pid).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn discover_from_registry_reaps_stale_foreign_server_owned_daemon() {
    let python = require_python!();
    let state_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvVarGuard::set_path("BATCHALIGN_STATE_DIR", state_dir.path());
    let registry_path = registry_path_for(&state_dir);

    let stale_owned = WorkerConfig {
        runtime: WorkerRuntimeConfig {
            server_instance_id: Some("dead-owner-instance".to_string()),
            server_process_id: Some(4_000_000),
            ..Default::default()
        },
        ..test_echo_tcp_config(python.clone())
    };
    let (stale_pid, _stale_port) = spawn_tcp_daemon(&stale_owned, 0)
        .await
        .expect("spawn stale server-owned tcp daemon");

    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: WorkerRuntimeConfig {
            server_instance_id: Some("current-server-instance".to_string()),
            server_process_id: Some(std::process::id()),
            ..Default::default()
        },
        worker_registry_path: registry_path.to_string_lossy().into_owned(),
        ..Default::default()
    });

    let discovered = pool.discover_from_registry().await;
    assert_eq!(
        discovered, 0,
        "orphaned server-owned daemons should be reaped, not reused"
    );
    wait_for_process_exit(stale_pid).await;

    let entries = read_registry(&registry_path);
    assert!(
        entries.is_empty(),
        "stale server-owned registry entries should be removed: {entries:?}"
    );

    pool.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_only_kills_current_server_owned_daemons() {
    let python = require_python!();
    let state_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvVarGuard::set_path("BATCHALIGN_STATE_DIR", state_dir.path());
    let registry_path = registry_path_for(&state_dir);

    let external = test_echo_tcp_config(python.clone());
    let (external_pid, _external_port) = spawn_tcp_daemon(&external, 0)
        .await
        .expect("spawn external tcp daemon");

    let owned_runtime = WorkerRuntimeConfig {
        server_instance_id: Some("owning-server-instance".to_string()),
        server_process_id: Some(std::process::id()),
        ..Default::default()
    };
    let owned = WorkerConfig {
        runtime: owned_runtime.clone(),
        ..test_echo_tcp_config(python.clone())
    };
    let (owned_pid, _owned_port) = spawn_tcp_daemon(&owned, 0)
        .await
        .expect("spawn server-owned tcp daemon");

    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: owned_runtime,
        worker_registry_path: registry_path.to_string_lossy().into_owned(),
        ..Default::default()
    });

    pool.shutdown().await;
    wait_for_process_exit(owned_pid).await;
    assert!(
        process_alive(external_pid),
        "shutdown should preserve external registry daemons"
    );

    let entries = read_registry(&registry_path);
    assert_eq!(
        entries.len(),
        1,
        "expected only the external daemon to remain"
    );
    assert_eq!(entries[0].pid, external_pid);
    assert_eq!(entries[0].ownership, RegistryOwnership::External);

    terminate_pid(external_pid);
    wait_for_process_exit(external_pid).await;
}

#[tokio::test]
async fn pool_pre_scale_respects_max_workers_per_key() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(2),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    pool.pre_scale(
        ReleasedCommand::Morphotag,
        WorkerLanguage::from(LanguageCode3::eng()),
        4,
    )
    .await;
    let count = pool.worker_count().await;
    assert!(
        count <= 2,
        "Expected at most 2 workers (max_workers_per_key=2), got {count}"
    );

    pool.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn pool_serializes_worker_bootstrap_per_key() {
    let python = require_python!();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let wrapped_python = dir.path().join("wrapped-python");
    std::fs::write(
        &wrapped_python,
        format!(
            "#!/bin/sh\nsleep 0.5\nexec \"{}\" \"$@\"\n",
            python.replace('"', "\\\"")
        ),
    )
    .expect("write wrapped python");
    let mut perms = std::fs::metadata(&wrapped_python)
        .expect("metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&wrapped_python, perms).expect("chmod wrapped python");

    let pool = WorkerPool::new(PoolConfig {
        python_path: wrapped_python.to_string_lossy().into_owned(),
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(3),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    let item1 = json!({"request": 1});
    let item2 = json!({"request": 2});
    let item3 = json!({"request": 3});
    let lang = LanguageCode3::eng();
    let request1 = batch_request(InferTask::Morphosyntax, vec![item1.clone()]);
    let request2 = batch_request(InferTask::Morphosyntax, vec![item2.clone()]);
    let request3 = batch_request(InferTask::Morphosyntax, vec![item3.clone()]);
    let started = tokio::time::Instant::now();
    let (r1, r2, r3) = tokio::join!(
        pool.dispatch_batch_infer(&lang, &request1),
        pool.dispatch_batch_infer(&lang, &request2),
        pool.dispatch_batch_infer(&lang, &request3),
    );
    let elapsed = started.elapsed();

    assert_eq!(
        r1.expect("dispatch 1 failed").results[0].result,
        Some(item1)
    );
    assert_eq!(
        r2.expect("dispatch 2 failed").results[0].result,
        Some(item2)
    );
    assert_eq!(
        r3.expect("dispatch 3 failed").results[0].result,
        Some(item3)
    );
    assert_eq!(pool.worker_count().await, 3);
    assert!(
        elapsed >= std::time::Duration::from_millis(1100),
        "expected serialized bootstrap to take at least 1.1s, got {:?}",
        elapsed
    );

    pool.shutdown().await;
}

#[tokio::test]
async fn spawn_failure_bad_python_path() {
    let config = WorkerConfig {
        python_path: "/nonexistent/python3".to_string(),
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let err = match WorkerHandle::spawn(config).await {
        Err(e) => e,
        Ok(_) => panic!("expected spawn to fail with bad python path"),
    };
    assert!(
        matches!(err, WorkerError::SpawnFailed(_)),
        "expected SpawnFailed, got: {err}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_tolerates_non_json_stdout_preamble_before_ready() {
    common::test_server_fixture::isolate_host_memory_ledger();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let fake_python = dir.path().join("fake-python");
    std::fs::write(
        &fake_python,
        "#!/bin/sh\nprintf 'Downloading: \"https://example.invalid/model.pt\" to /tmp/model.pt\\n'\nprintf '{\"ready\":true,\"pid\":1234,\"transport\":\"stdio\"}\\n'\nsleep 30\n",
    )
    .expect("write fake python");
    let mut perms = std::fs::metadata(&fake_python)
        .expect("metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake_python, perms).expect("chmod fake python");

    let config = WorkerConfig {
        python_path: fake_python.to_string_lossy().into_owned(),
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let handle = WorkerHandle::spawn(config).await.expect("spawn failed");
    assert!(*handle.pid() > 0, "should have a valid pid");
    assert_eq!(handle.transport(), "stdio");
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_failure_includes_worker_startup_stderr() {
    common::test_server_fixture::isolate_host_memory_ledger();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let fake_python = dir.path().join("fake-python");
    std::fs::write(
        &fake_python,
        "#!/bin/sh\nprintf 'synthetic worker startup failure\\n' >&2\nexit 23\n",
    )
    .expect("write fake python");
    let mut perms = std::fs::metadata(&fake_python)
        .expect("metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake_python, perms).expect("chmod fake python");

    let config = WorkerConfig {
        python_path: fake_python.to_string_lossy().into_owned(),
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let err = match WorkerHandle::spawn(config).await {
        Err(e) => e,
        Ok(_) => panic!("expected spawn to fail with synthetic stderr"),
    };
    match err {
        WorkerError::ReadyParseFailed(message) => {
            assert!(
                message.contains("worker closed stdout without emitting ready signal"),
                "missing ready failure detail: {message}"
            );
            assert!(
                message.contains("synthetic worker startup failure"),
                "missing worker stderr detail: {message}"
            );
        }
        other => panic!("expected ReadyParseFailed, got: {other}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn health_check_tolerates_non_protocol_stdout_between_requests() {
    common::test_server_fixture::isolate_host_memory_ledger();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let fake_python = dir.path().join("fake-python");
    std::fs::write(
        &fake_python,
        "#!/bin/sh\nprintf '{\"ready\":true,\"pid\":1234,\"transport\":\"stdio\"}\\n'\nIFS= read -r req || exit 1\nprintf 'torch: loading checkpoint shards\\n'\nprintf '{\"op\":\"health\",\"response\":{\"status\":\"ok\",\"command\":\"profile:stanza\",\"lang\":\"eng\",\"pid\":1234,\"uptime_s\":0}}\\n'\nIFS= read -r req || exit 0\nprintf '{\"op\":\"shutdown\"}\\n'\n",
    )
    .expect("write fake python");
    let mut perms = std::fs::metadata(&fake_python)
        .expect("metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake_python, perms).expect("chmod fake python");

    let config = WorkerConfig {
        python_path: fake_python.to_string_lossy().into_owned(),
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let mut handle = WorkerHandle::spawn(config).await.expect("spawn failed");
    let health = handle.health_check().await.expect("health check failed");
    assert_eq!(health.status, batchalign::worker::WorkerHealthStatus::Ok);
    assert_eq!(health.command, "profile:stanza");
    assert_eq!(health.lang, WorkerLanguage::from(LanguageCode3::eng()));

    handle.shutdown().await.expect("shutdown failed");
}

/// Two different InferTasks within the same profile share one worker.
#[tokio::test]
async fn profile_groups_related_tasks_into_single_worker() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    // Dispatch morphosyntax and utseg — both Stanza profile.
    let morph_item = json!({"task": "morph"});
    let utseg_item = json!({"task": ReleasedCommand::Utseg});
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(InferTask::Morphosyntax, vec![morph_item]),
    )
    .await
    .expect("morphosyntax dispatch failed");
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(InferTask::Utseg, vec![utseg_item]),
    )
    .await
    .expect("utseg dispatch failed");

    // Both should use the same Stanza worker — only 1 worker total.
    assert_eq!(
        pool.worker_count().await,
        1,
        "morphosyntax and utseg should share a single Stanza profile worker"
    );

    pool.shutdown().await;
}

/// Three different profiles produce exactly three workers.
#[tokio::test]
async fn each_profile_gets_its_own_worker() {
    let python = require_python!();
    let pool = WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 60,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    });

    // Dispatch one task from each profile via batch_infer.
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(InferTask::Morphosyntax, vec![json!({"p": "stanza"})]),
    )
    .await
    .expect("stanza dispatch failed");
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(InferTask::Translate, vec![json!({"p": "io"})]),
    )
    .await
    .expect("io dispatch failed");
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(InferTask::Fa, vec![json!({"p": "gpu"})]),
    )
    .await
    .expect("gpu dispatch failed");

    // Three different profiles -> three workers.
    assert_eq!(
        pool.worker_count().await,
        3,
        "expected one worker per profile (Stanza, IO, GPU)"
    );

    pool.shutdown().await;
}
