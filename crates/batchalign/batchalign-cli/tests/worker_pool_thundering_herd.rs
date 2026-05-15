// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! BUG-028 deeper — global-cap thundering herd.
//!
//! Spawns `max_total_workers + EXTRA` simultaneous dispatch tasks
//! against a pool sized to `max_total_workers`. Under the wake-and-probe
//! design, every worker return wakes ALL parked waiters via
//! `worker_returned.notify_waiters()` at `checkout.rs:105`; each
//! re-probes the global cap and most re-park, generating O(N)
//! rejections per worker return.
//!
//! After the semaphore refactor, FIFO-fair permits should produce only
//! the unavoidable first-attempt rejections (one per oversubscribed
//! caller) plus rare wakeup-races. The post-fix budget below is
//! generous enough to absorb the latter without false-positives, while
//! still failing clearly on the current wake-and-probe design.
//!
//! `test_delay_ms` is set so each worker response takes ~100 ms,
//! forcing all 16 oversubscribed callers to be simultaneously parked
//! before the first worker returns. Without the delay, test-echo
//! workers complete dispatches faster than callers can queue and the
//! herd never forms.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use batchalign::api::LanguageCode3;
use batchalign::host_facts::PerProfile;
use batchalign::worker::pool::WorkerPool;
use batchalign::worker::{BatchInferRequest, InferTask};
use common::pool_dispatch::{count_successes, echo_pool_config, launch_oversubscribed_dispatches};
use common::resolve_python;
use serde_json::json;

const MAX_TOTAL: usize = 4;
const EXTRA: usize = 16;

/// Generous post-fix budget: each oversubscribed caller may legitimately
/// be rejected on its first admission attempt (before parking on the
/// permit semaphore). FIFO wakeups produce ~zero extra rejections, but
/// runtime jitter and `try_acquire` races can leak a few. Total cap:
/// 1.5x oversubscription. Pre-fix code blows past this within a few
/// worker returns.
const REJECTION_BUDGET: u64 = ((MAX_TOTAL + EXTRA) * 3 / 2) as u64;

/// Per-worker artificial delay so the herd has time to form before
/// the first worker returns.
const TEST_DELAY_MS: u64 = 100;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_oversubscribe_does_not_storm_global_cap_rejections() {
    common::test_server_fixture::isolate_host_memory_ledger();
    let Some(python) = resolve_python() else {
        eprintln!("SKIP: Python 3 with batchalign not available");
        return;
    };

    let pool = std::sync::Arc::new(WorkerPool::new(echo_pool_config(
        python,
        MAX_TOTAL,
        PerProfile::uniform(MAX_TOTAL),
        TEST_DELAY_MS,
    )));

    // Four distinct languages so every dispatch creates its own group
    // and contends on the GLOBAL cap rather than a per-key cap.
    let langs = [
        LanguageCode3::eng(),
        LanguageCode3::spa(),
        LanguageCode3::fra(),
        LanguageCode3::deu(),
    ];

    let outcomes = launch_oversubscribed_dispatches(
        &pool,
        MAX_TOTAL + EXTRA,
        Duration::from_secs(60),
        |i| langs[i % langs.len()].clone(),
        |lang, _| BatchInferRequest {
            task: InferTask::Morphosyntax,
            lang: lang.clone(),
            items: vec![json!({"words": ["hello"], "lang": lang.as_ref()})],
            mwt: BTreeMap::new(),
            allow_stanza_fallback: false,
        },
    )
    .await;

    let completed = count_successes(&outcomes);
    for outcome in &outcomes {
        if !matches!(outcome, Ok(Ok(Ok(_)))) {
            eprintln!("dispatch outcome: {outcome:?}");
        }
    }
    assert!(
        completed >= MAX_TOTAL + EXTRA - 4,
        "expected most dispatches to complete; got {completed}/{}",
        MAX_TOTAL + EXTRA
    );

    let metrics = pool.metrics_snapshot();
    assert!(
        metrics.spawn_rejections_total <= REJECTION_BUDGET,
        "thundering herd: got {} global-cap rejections (budget {REJECTION_BUDGET}); \
         metrics={metrics:?}",
        metrics.spawn_rejections_total,
    );

    pool.shutdown().await;
}
