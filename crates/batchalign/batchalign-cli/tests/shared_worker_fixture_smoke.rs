// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Smoke tests for the shared `test_worker_pool` fixture.
//!
//! Pin three behaviors of the fixture in isolation, before any production
//! test binary depends on it:
//!
//! - Two checkouts with the same effective `WorkerConfig` reuse the same
//!   underlying worker process (same pid).
//! - Two checkouts with `WorkerConfig`s that differ in any Python-observed
//!   field spawn distinct workers (distinct pids).
//! - `WorkerConfig` fields the Python child does NOT observe at startup
//!   (e.g. `ready_timeout_s`) do not partition the pool — checkouts that
//!   differ only in such a field still share a worker.

mod common;

use batchalign::api::{LanguageCode3, NumSpeakers, WorkerLanguage};
use batchalign::worker::WorkerProfile;
use batchalign::worker::handle::WorkerConfig;
use common::resolve_python;
use common::test_worker_pool::shared_test_worker_pool;

/// Skip when Python is unavailable or memory is tight, matching the rest of
/// the python-workers test binaries.
macro_rules! require_python {
    () => {{
        common::test_server_fixture::isolate_host_memory_ledger();
        let available_mb = batchalign::worker::memory_guard::available_memory_mb();
        if available_mb < 4096 {
            eprintln!("SKIP: insufficient memory ({available_mb} MB available, 4096 MB required).");
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

fn echo_config(python: String) -> WorkerConfig {
    WorkerConfig {
        python_path: python,
        profile: WorkerProfile::Stanza,
        lang: WorkerLanguage::from(LanguageCode3::eng()),
        num_speakers: NumSpeakers(1),
        test_echo: true,
        ready_timeout_s: 30,
        ..Default::default()
    }
}

#[tokio::test]
async fn checkout_same_config_returns_same_worker() {
    let python = require_python!();
    let pool = shared_test_worker_pool();

    let pid_a = {
        let lease = pool
            .checkout(&echo_config(python.clone()))
            .await
            .expect("first checkout failed");
        lease.pid()
    };

    let pid_b = {
        let lease = pool
            .checkout(&echo_config(python))
            .await
            .expect("second checkout failed");
        lease.pid()
    };

    assert_eq!(
        pid_a, pid_b,
        "shared pool should reuse the same worker for identical configs"
    );
}

#[tokio::test]
async fn checkout_distinct_python_observed_field_spawns_distinct_worker() {
    let python = require_python!();
    let pool = shared_test_worker_pool();

    let cfg_a = echo_config(python.clone());
    // `test_delay_ms` is forwarded to Python as `--test-delay-ms`, so two
    // configs differing only in this field must yield distinct workers.
    let mut cfg_b = echo_config(python);
    cfg_b.test_delay_ms = 50;

    let pid_a = {
        let lease = pool.checkout(&cfg_a).await.expect("checkout A failed");
        lease.pid()
    };
    let pid_b = {
        let lease = pool.checkout(&cfg_b).await.expect("checkout B failed");
        lease.pid()
    };

    assert_ne!(
        pid_a, pid_b,
        "configs differing in a Python-observed field must spawn distinct workers"
    );
}

#[tokio::test]
async fn checkout_distinct_rust_only_field_still_shares_worker() {
    let python = require_python!();
    let pool = shared_test_worker_pool();

    // `ready_timeout_s` is consumed Rust-side in `tokio::time::timeout` and
    // never reaches the Python child. Differing values must NOT partition
    // the pool.
    let cfg_a = WorkerConfig {
        ready_timeout_s: 30,
        ..echo_config(python.clone())
    };
    let cfg_b = WorkerConfig {
        ready_timeout_s: 90,
        ..echo_config(python)
    };

    let pid_a = {
        let lease = pool.checkout(&cfg_a).await.expect("checkout A failed");
        lease.pid()
    };
    let pid_b = {
        let lease = pool.checkout(&cfg_b).await.expect("checkout B failed");
        lease.pid()
    };

    assert_eq!(
        pid_a, pid_b,
        "configs differing only in Rust-side fields should share a worker"
    );
}
