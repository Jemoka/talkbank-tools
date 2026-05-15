// Integration test target. Tests use unwrap/expect by convention; the
// lib's `cfg_attr(test, ...)` allow does not apply here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Per-key worker count throughput sweep for the Stanza profile.
//!
//! [`recommend_max_workers_per_key`] derives per-profile worker counts
//! from RAM only:
//!
//! ```text
//! gpu    = clamp(ram / 16 GB, 1, 8)
//! stanza = clamp(ram / 12 GB, 1, 8)
//! io     = 1
//! ```
//!
//! Hosts with abundant RAM but limited core count can find the
//! RAM-only formula recommends more Stanza workers than their cores
//! actually support — every Stanza worker is a CPU-bound process, so
//! oversubscribing cores costs throughput rather than buying it. This
//! test exercises that question empirically: sweep
//! `max_workers_per_key.stanza` while holding everything else fixed,
//! dispatch a fixed number of concurrent batched-infer requests, and
//! record the wall-clock to all-complete. The output line per
//! invocation is consumed by an external orchestrator that aggregates
//! sweep points into a throughput-vs-K plot.
//!
//! Run is gated by `#[ignore]` because each sweep point cold-loads `K`
//! Stanza workers (~30 s each on a CPU build) and the Stanza Python
//! environment may not be available everywhere.
//!
//! ## How to run
//!
//! Single-point invocation (set `STANZA_PER_KEY` from `1..=9`):
//!
//! ```text
//! STANZA_PER_KEY=7 cargo nextest run -p batchalign-cli \
//!     --test pool_per_key_worker_throughput -- --ignored
//! ```
//!
//! Each invocation prints exactly one machine-parseable line on
//! stdout:
//!
//! ```text
//! POOL_THROUGHPUT_RESULT,stanza_per_key=K,elapsed_ms=<ms>,\
//!   permit_rejections=<n>,spawn_rejections=<n>,completed=<n>/<total>
//! ```
//!
//! Aggregate those lines across runs to plot the throughput-vs-K
//! curve and locate the knee for a particular host shape.

mod common;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use batchalign::api::LanguageCode3;
use batchalign::host_facts::PerProfile;
use batchalign::worker::pool::{PoolConfig, WorkerPool};
use batchalign::worker::{BatchInferRequest, InferTask};
use common::pool_dispatch::{count_successes, launch_oversubscribed_dispatches};
use common::resolve_python;
use serde_json::json;

/// Concurrent dispatches launched per sweep point. Must oversubscribe
/// `K` for any value of K we plan to test, so the per-key cap is the
/// binding constraint and we observe the cost of the queueing imposed
/// by it. With `K_MAX = 9` (the largest sweep value), we want
/// `CONCURRENT >= 16` to keep at least 7 callers parked when 9 workers
/// are busy.
const CONCURRENT_DISPATCHES: usize = 16;

/// Items per request. Each item is a synthetic short utterance — words
/// list of fixed length. Stanza tokenizes + tags each item; the
/// per-request work scales linearly with item count.
///
/// Sized so that one request takes long enough (~few hundred ms on
/// loaded workers) to make queueing-vs-parallelism behavior visible
/// without inflating total sweep runtime. Adjust if the test ends up
/// either too noisy (too short) or too slow (too long).
const ITEMS_PER_REQUEST: usize = 50;

/// Per-request timeout. With K=3 workers and 16 callers a typical
/// run queues most callers behind ~5 batches, each batch potentially
/// hundreds of ms; 90 s gives plenty of headroom even at the slow
/// end of the sweep without silently passing on a hung pool.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);

/// Read the per-key Stanza budget from `STANZA_PER_KEY`, defaulting
/// to a moderate `7` when unset. Reject obvious nonsense up-front
/// so a typo never silently picks an unintended configuration.
fn stanza_per_key_from_env() -> usize {
    let raw = std::env::var("STANZA_PER_KEY").unwrap_or_else(|_| "7".to_owned());
    let k: usize = raw
        .parse()
        .unwrap_or_else(|_| panic!("STANZA_PER_KEY must be a positive integer; got {raw:?}"));
    assert!(
        (1..=16).contains(&k),
        "STANZA_PER_KEY out of plausible range; got {k}"
    );
    k
}

/// Build one synthetic batched-infer item — a list of `words` to be
/// tagged by Stanza, paired with a per-item `lang` field as required by
/// the worker protocol. The vocabulary is fixed-but-realistic so each
/// item exercises tokenize + POS + lemma + depparse stages; varying
/// vocabulary across items would just add noise.
fn synthetic_item(lang: LanguageCode3) -> serde_json::Value {
    json!({
        "words": [
            "the", "child", "wanted", "to", "tell", "the", "story",
            "about", "her", "favorite", "trip", ".",
        ],
        "lang": lang.as_ref(),
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "Real Stanza models; opt-in via --ignored. Reads STANZA_PER_KEY."]
async fn measure_stanza_per_key_throughput() {
    common::test_server_fixture::isolate_host_memory_ledger();

    let Some(python) = resolve_python() else {
        eprintln!("SKIP: Python 3 with batchalign not available");
        return;
    };

    let stanza_k = stanza_per_key_from_env();
    eprintln!(
        "Sweep point: stanza_per_key={stanza_k} \
         (CONCURRENT_DISPATCHES={CONCURRENT_DISPATCHES}, \
         ITEMS_PER_REQUEST={ITEMS_PER_REQUEST})"
    );

    // Pool sized at the configured per-key value with global capacity
    // generous enough not to be the binding constraint. We are isolating
    // the per-key axis: gpu and io stay at 1 so they cannot absorb
    // headroom and confound the measurement.
    let pool = std::sync::Arc::new(WorkerPool::new(PoolConfig {
        python_path: python,
        max_total_workers: stanza_k.max(8),
        max_workers_per_key: PerProfile {
            gpu: 1,
            stanza: stanza_k,
            io: 1,
        },
        test_echo: false,
        // Long timeouts so cold model loads (~30 s) on every spawn
        // don't trip the readiness gate. We're measuring steady-state
        // throughput, not warmup.
        health_check_interval_s: 600,
        ready_timeout_s: 180,
        ..Default::default()
    }));

    let lang = LanguageCode3::eng();

    // All dispatches share the same (profile=stanza, lang=eng,
    // no engine_overrides) key, so the per-key cap is the only thing
    // gating concurrency.
    let started = Instant::now();
    let outcomes = launch_oversubscribed_dispatches(
        &pool,
        CONCURRENT_DISPATCHES,
        REQUEST_TIMEOUT,
        |_| lang.clone(),
        |lang, _| BatchInferRequest {
            task: InferTask::Morphosyntax,
            lang: lang.clone(),
            items: (0..ITEMS_PER_REQUEST)
                .map(|_| synthetic_item(lang.clone()))
                .collect(),
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
    let elapsed = started.elapsed();
    let metrics = pool.metrics_snapshot();

    // The single machine-parseable result line. The shell loop in
    // `scripts/run_bug_030_sweep.sh` greps stdout for this prefix and
    // appends it to a CSV; keep the format stable.
    println!(
        "POOL_THROUGHPUT_RESULT,stanza_per_key={stanza_k},elapsed_ms={},\
         permit_rejections={},spawn_rejections={},completed={}/{}",
        elapsed.as_millis(),
        metrics.permit_rejections_total,
        metrics.spawn_rejections_total,
        completed,
        CONCURRENT_DISPATCHES
    );

    pool.shutdown().await;

    // Sanity: assert most dispatches actually finished. The throughput
    // measurement only means something if the pool serviced the load —
    // a config that drops half the requests is not a "fast" config.
    assert!(
        completed >= CONCURRENT_DISPATCHES * 3 / 4,
        "expected most dispatches to complete; got {completed}/{CONCURRENT_DISPATCHES}"
    );
}
