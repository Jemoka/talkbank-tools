// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Tests for concurrent worker routing (T080), cross-platform process management
//! (T081), and platform-specific lifecycle/shutdown behavior (T089).
//!
//! T080: Multi-language dispatch creates separate groups; engine overrides create
//!       distinct groups; concurrent dispatch across groups works correctly.
//! T081: Worker spawn, health check, and shutdown work on the current platform.
//! T089: Graceful shutdown sends SIGTERM (Unix); Drop reaps processes; checked-out
//!       workers are warned during shutdown.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use batchalign::api::{LanguageCode3, WorkerLanguage};
use batchalign::host_facts::PerProfile;
use batchalign::worker::handle::{WorkerConfig, WorkerHandle};
use batchalign::worker::pool::{PoolConfig, WorkerPool};
use batchalign::worker::{BatchInferRequest, InferTask, WorkerProfile};
use common::resolve_python;
use serde_json::{Value, json};

macro_rules! require_python {
    () => {{
        common::test_server_fixture::isolate_host_memory_ledger();
        let available_mb = batchalign::worker::memory_guard::available_memory_mb();
        if available_mb < 4096 {
            eprintln!("SKIP: insufficient memory ({available_mb} MB available, 4096 MB required)");
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

fn batch_request(task: InferTask, lang: LanguageCode3, items: Vec<Value>) -> BatchInferRequest {
    BatchInferRequest {
        task,
        lang,
        items,
        mwt: BTreeMap::new(),
        allow_stanza_fallback: false,
    }
}

fn test_pool(python: String) -> WorkerPool {
    common::test_server_fixture::isolate_host_memory_ledger();
    WorkerPool::new(PoolConfig {
        python_path: python,
        health_check_interval_s: 600,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// T080: Concurrent worker routing tests
// ---------------------------------------------------------------------------

/// Concurrent dispatch to two different languages should create separate worker
/// groups and both should succeed without cross-contamination.
#[tokio::test]
async fn concurrent_dispatch_to_different_languages() {
    let python = require_python!();
    let pool = test_pool(python);

    let eng_item = json!({"words": ["hello"], "lang": "eng"});
    let spa_item = json!({"words": ["hola"], "lang": "spa"});

    // Dispatch to both languages concurrently.
    let pool_ptr = &pool as *const WorkerPool as usize;
    let eng_handle = tokio::spawn({
        let eng_item_clone = eng_item.clone();
        async move {
            let pool = unsafe { &*(pool_ptr as *const WorkerPool) };
            pool.dispatch_batch_infer(
                &LanguageCode3::eng(),
                &batch_request(
                    InferTask::Morphosyntax,
                    LanguageCode3::eng(),
                    vec![eng_item_clone],
                ),
            )
            .await
        }
    });
    let spa_handle = tokio::spawn({
        let spa_item_clone = spa_item.clone();
        async move {
            let pool = unsafe { &*(pool_ptr as *const WorkerPool) };
            pool.dispatch_batch_infer(
                &LanguageCode3::spa(),
                &batch_request(
                    InferTask::Morphosyntax,
                    LanguageCode3::spa(),
                    vec![spa_item_clone],
                ),
            )
            .await
        }
    });

    let eng_result = eng_handle.await.expect("eng task panicked");
    let spa_result = spa_handle.await.expect("spa task panicked");

    let eng_resp = eng_result.expect("eng dispatch failed");
    let spa_resp = spa_result.expect("spa dispatch failed");
    assert_eq!(eng_resp.results[0].result, Some(eng_item));
    assert_eq!(spa_resp.results[0].result, Some(spa_item));

    // Two different languages → two separate workers.
    assert_eq!(
        pool.worker_count().await,
        2,
        "two languages should produce two workers"
    );

    pool.shutdown().await;
}

/// Engine overrides create a distinct worker group even for the same task+language.
#[tokio::test]
async fn engine_overrides_create_distinct_worker_groups() {
    let python = require_python!();

    // Pool with engine overrides.
    let pool_with_overrides = WorkerPool::new(PoolConfig {
        python_path: python.clone(),
        health_check_interval_s: 600,
        ready_timeout_s: 30,
        test_echo: true,
        max_workers_per_key: PerProfile::uniform(8),
        verbose: 0,
        engine_overrides: r#"{"asr":"tencent"}"#.into(),
        runtime: Default::default(),
        ..Default::default()
    });

    // Pool without engine overrides.
    let pool_default = test_pool(python);

    // Dispatch to both.
    let item = json!({"words": ["test"]});
    pool_with_overrides
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(
                InferTask::Morphosyntax,
                LanguageCode3::eng(),
                vec![item.clone()],
            ),
        )
        .await
        .expect("override pool dispatch failed");

    pool_default
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(InferTask::Morphosyntax, LanguageCode3::eng(), vec![item]),
        )
        .await
        .expect("default pool dispatch failed");

    // Each pool should have its own worker.
    assert_eq!(pool_with_overrides.worker_count().await, 1);
    assert_eq!(pool_default.worker_count().await, 1);

    // Workers should be distinct (different PIDs).
    let summary_overrides = pool_with_overrides.worker_summary().await;
    let summary_default = pool_default.worker_summary().await;
    assert_ne!(
        summary_overrides, summary_default,
        "workers with different engine overrides should be distinct"
    );

    pool_with_overrides.shutdown().await;
    pool_default.shutdown().await;
}

/// Concurrent dispatch to multiple task types (Morphosyntax, Translate, FA)
/// should all succeed simultaneously without blocking each other.
#[tokio::test]
async fn concurrent_dispatch_across_task_types() {
    let python = require_python!();
    let pool = test_pool(python);

    let pool_ptr = &pool as *const WorkerPool as usize;

    let morph_handle = tokio::spawn(async move {
        let pool = unsafe { &*(pool_ptr as *const WorkerPool) };
        pool.dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(
                InferTask::Morphosyntax,
                LanguageCode3::eng(),
                vec![json!({"task": "morph"})],
            ),
        )
        .await
    });
    let translate_handle = tokio::spawn(async move {
        let pool = unsafe { &*(pool_ptr as *const WorkerPool) };
        pool.dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(
                InferTask::Translate,
                LanguageCode3::eng(),
                vec![json!({"task": "translate"})],
            ),
        )
        .await
    });
    let fa_handle = tokio::spawn(async move {
        let pool = unsafe { &*(pool_ptr as *const WorkerPool) };
        pool.dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(
                InferTask::Fa,
                LanguageCode3::eng(),
                vec![json!({"task": "fa"})],
            ),
        )
        .await
    });

    let (morph, translate, fa) = tokio::join!(morph_handle, translate_handle, fa_handle);
    morph
        .expect("morph panicked")
        .expect("morph dispatch failed");
    translate
        .expect("translate panicked")
        .expect("translate dispatch failed");
    fa.expect("fa panicked").expect("fa dispatch failed");

    // Three different profiles → three workers.
    assert_eq!(
        pool.worker_count().await,
        3,
        "three task types should produce three workers"
    );

    pool.shutdown().await;
}

// ---------------------------------------------------------------------------
// T081: Cross-platform startup and process-management behavior tests
// ---------------------------------------------------------------------------

/// Worker spawn and shutdown work on the current platform.
/// This test is platform-agnostic — it verifies the basic lifecycle.
#[tokio::test]
async fn cross_platform_worker_spawn_and_shutdown() {
    let python = require_python!();
    let config = WorkerConfig {
        python_path: python,
        test_echo: true,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        ready_timeout_s: 30,
        ..Default::default()
    };

    let handle = WorkerHandle::spawn(config).await.expect("spawn failed");
    assert!(*handle.pid() > 0, "should have a valid pid");
    assert_eq!(handle.transport(), "stdio");

    handle
        .shutdown()
        .await
        .expect("shutdown should succeed on all platforms");
}

/// Pool creation, dispatch, and shutdown form a complete lifecycle on any platform.
#[tokio::test]
async fn cross_platform_pool_lifecycle() {
    let python = require_python!();
    let pool = test_pool(python);

    // Dispatch.
    let item = json!({"words": ["platform", "test"]});
    let response = pool
        .dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(
                InferTask::Morphosyntax,
                LanguageCode3::eng(),
                vec![item.clone()],
            ),
        )
        .await
        .expect("dispatch failed");
    assert_eq!(response.results[0].result, Some(item));
    assert_eq!(pool.worker_count().await, 1);

    // Shutdown.
    pool.shutdown().await;
    assert_eq!(pool.worker_count().await, 0);
}

/// Worker health check works on the current platform.
#[tokio::test]
async fn cross_platform_worker_health_check() {
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
}

/// Worker capabilities detection works on the current platform.
#[tokio::test]
async fn cross_platform_worker_capabilities() {
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
    assert!(
        caps.commands.iter().any(|c| c == "test-echo"),
        "test-echo worker should report test-echo capability"
    );
}

// ---------------------------------------------------------------------------
// T089: Platform-specific lifecycle tests for shutdown and cleanup
// ---------------------------------------------------------------------------

/// Graceful pool shutdown kills idle workers (process exits).
#[cfg(unix)]
#[tokio::test]
async fn shutdown_kills_idle_workers() {
    let python = require_python!();
    let pool = test_pool(python);

    // Spawn a worker by dispatching.
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(
            InferTask::Morphosyntax,
            LanguageCode3::eng(),
            vec![json!({"words": ["shutdown-test"]})],
        ),
    )
    .await
    .expect("dispatch failed");

    // Get the worker PID before shutdown.
    let summary = pool.worker_summary().await;
    let pid = summary
        .first()
        .expect("should have a worker")
        .split(':')
        .find_map(|part| part.strip_prefix("pid="))
        .and_then(|p| p.parse::<u32>().ok())
        .expect("should parse pid");

    assert!(process_alive(pid), "worker should be alive before shutdown");

    pool.shutdown().await;

    // Worker should be dead after shutdown.
    wait_for_process_exit(pid).await;
}

/// Dropping the pool without calling `shutdown()` still reaps worker processes.
/// This tests the Drop impl safety net.
#[cfg(unix)]
#[tokio::test]
async fn drop_without_shutdown_still_reaps_workers() {
    let python = require_python!();
    let pid = {
        let pool = test_pool(python);
        pool.dispatch_batch_infer(
            &LanguageCode3::eng(),
            &batch_request(
                InferTask::Morphosyntax,
                LanguageCode3::eng(),
                vec![json!({"words": ["drop-test"]})],
            ),
        )
        .await
        .expect("dispatch failed");

        let summary = pool.worker_summary().await;
        let pid = summary
            .first()
            .expect("should have a worker")
            .split(':')
            .find_map(|part| part.strip_prefix("pid="))
            .and_then(|p| p.parse::<u32>().ok())
            .expect("should parse pid");
        assert!(process_alive(pid), "worker should be alive");

        drop(pool);
        pid
    };

    wait_for_process_exit(pid).await;
}

/// Multiple workers from different groups are all cleaned up on shutdown.
#[cfg(unix)]
#[tokio::test]
async fn shutdown_cleans_up_multiple_worker_groups() {
    let python = require_python!();
    let pool = test_pool(python);

    // Create workers in 3 different profiles.
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(
            InferTask::Morphosyntax,
            LanguageCode3::eng(),
            vec![json!({"p": "stanza"})],
        ),
    )
    .await
    .expect("stanza dispatch");
    pool.dispatch_batch_infer(
        &LanguageCode3::eng(),
        &batch_request(
            InferTask::Translate,
            LanguageCode3::eng(),
            vec![json!({"p": "io"})],
        ),
    )
    .await
    .expect("io dispatch");

    assert_eq!(
        pool.worker_count().await,
        2,
        "should have 2 workers before shutdown"
    );

    // Collect all PIDs.
    let summary = pool.worker_summary().await;
    let pids: Vec<u32> = summary
        .iter()
        .filter_map(|s| {
            s.split(':')
                .find_map(|part| part.strip_prefix("pid="))
                .and_then(|p| p.parse::<u32>().ok())
        })
        .collect();
    assert_eq!(pids.len(), 2, "should have 2 worker PIDs");

    for &pid in &pids {
        assert!(
            process_alive(pid),
            "worker pid {pid} should be alive before shutdown"
        );
    }

    pool.shutdown().await;

    // All workers should be dead.
    for pid in pids {
        wait_for_process_exit(pid).await;
    }
}

// ---------------------------------------------------------------------------
// Unix helpers
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(unix)]
async fn wait_for_process_exit(pid: u32) {
    for _ in 0..50 {
        if !process_alive(pid) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("worker pid {pid} was still alive after waiting for shutdown");
}
