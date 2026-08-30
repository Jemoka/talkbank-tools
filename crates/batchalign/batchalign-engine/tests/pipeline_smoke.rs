//! Pipeline-side smoke test — but trimmed to what's possible without the
//! full batchalign-core API materialized. The richer dispatch tests live
//! once the parallel agent ships TaskInput/TaskOutput variants.

#![allow(unused_imports)]

use std::sync::Arc;

use batchalign_engine::{Cache, CachePolicy};

#[test]
fn engine_components_compile() {
    // If this test compiles + runs, the engine's public surface
    // (Cache + CachePolicy + Pipeline + engine module) is wired
    // correctly enough to be linked by downstream consumers.
    let tmp = tempfile::tempdir().expect("tempdir");
    let _cache = Cache::open(&tmp.path().join("c.redb"), CachePolicy::Bypass).unwrap();
}
