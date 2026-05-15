// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Smoke tests for the shared test-server fixture.
//!
//! Pin three behaviors of the fixture before any production test binary
//! depends on it:
//!
//! - Two acquired sessions return distinct `base_url`s and distinct
//!   `state_dir`s — control-plane state cannot bleed between tests.
//! - Both sessions share a single warmed `WorkerPool`: across both
//!   acquires combined, `prepare_workers` runs at most once.
//! - Each session's `/health` endpoint responds 200, proving the axum
//!   server is actually listening on the reported port.

mod common;

use common::test_server_fixture::{acquire_test_server_session, times_prepared};

/// Skip cleanly when Python is unavailable, matching the rest of the
/// python-workers test binaries.
macro_rules! require_python_or_skip {
    () => {{
        if common::resolve_python().is_none() {
            eprintln!("SKIP: Python 3 with batchalign not available");
            return;
        }
    }};
}

#[tokio::test]
async fn two_sessions_have_distinct_state_dirs_and_base_urls() {
    require_python_or_skip!();

    let session_a = acquire_test_server_session()
        .await
        .expect("first acquire should succeed");
    let session_b = acquire_test_server_session()
        .await
        .expect("second acquire should succeed");

    assert_ne!(
        session_a.base_url(),
        session_b.base_url(),
        "two acquired sessions must report distinct base URLs"
    );
    assert_ne!(
        session_a.state_dir(),
        session_b.state_dir(),
        "two acquired sessions must own distinct state directories"
    );
}

#[tokio::test]
async fn two_sessions_share_one_warm_pool() {
    require_python_or_skip!();

    // First acquire warms the pool. Second acquire must reuse it.
    let _session_a = acquire_test_server_session()
        .await
        .expect("first acquire should succeed");
    let _session_b = acquire_test_server_session()
        .await
        .expect("second acquire should succeed");

    assert_eq!(
        times_prepared(),
        1,
        "shared fixture must invoke prepare_workers exactly once across multiple sessions"
    );
}

#[tokio::test]
async fn session_health_endpoint_is_reachable() {
    require_python_or_skip!();

    let session = acquire_test_server_session()
        .await
        .expect("acquire should succeed");

    let resp = session
        .client()
        .get(format!("{}/health", session.base_url()))
        .send()
        .await
        .expect("GET /health should succeed");
    assert_eq!(resp.status(), 200, "health endpoint should respond 200");
}
