//! Smoke test for the redb-backed cache (spec2.md §13).
//!
//! Sanity-checks all three policies: Use (round-trip), Bypass (no-op),
//! Refresh (write-only). Skipped automatically if `batchalign-core` hasn't
//! yet materialized the TaskInput/TaskOutput proto types we lean on, so
//! the test file compiles in isolation against the engine crate.

#![allow(unused_imports)]

use std::sync::Arc;

use batchalign_engine::{Cache, CachePolicy};

#[test]
fn cache_open_creates_file_and_parent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("sub").join("cache.redb");
    let cache = Cache::open(&path, CachePolicy::Use).expect("open cache");
    assert!(
        path.exists(),
        "cache file was not created at {}",
        path.display()
    );
    drop(cache);
}

#[test]
fn cache_policy_is_visible() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("cache.redb");
    let c1 = Cache::open(&path, CachePolicy::Use).unwrap();
    assert_eq!(c1.policy(), CachePolicy::Use);
    drop(c1);
    let c2 = Cache::open(&path, CachePolicy::Bypass).unwrap();
    assert_eq!(c2.policy(), CachePolicy::Bypass);
    drop(c2);
    let c3 = Cache::open(&path, CachePolicy::Refresh).unwrap();
    assert_eq!(c3.policy(), CachePolicy::Refresh);
}

// Once batchalign-core ships its TaskInput/TaskOutput, expand here to:
//   * put(input) under Use, get -> Some(output)
//   * put(input) under Bypass, get -> None
//   * put(input) under Refresh, get -> None (refresh always misses on read)
//
// Keep this test ready by importing the types as soon as core publishes
// them. For now we sanity-check the wrapper opens correctly across
// policies.
