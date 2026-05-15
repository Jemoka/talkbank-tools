// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Cross-language fixture checks for the proposed worker protocol V2.
//!
//! These tests do not exercise live worker execution directly. Their purpose
//! is narrower:
//!
//! - load the canonical V2 fixture files from the repo root
//! - deserialize them through the Rust schema types
//! - serialize them back to JSON
//! - prove the result matches the canonical fixture exactly
//!
//! The Python side runs the same fixture set through Pydantic models.

use std::fs;
use std::path::{Path, PathBuf};

use batchalign::types::worker_v2::{
    CapabilitiesRequestV2, CapabilitiesResponseV2, ExecuteRequestV2, ExecuteResponseV2,
    HelloRequestV2, HelloResponseV2, ProgressEventV2, ShutdownRequestV2,
};
use serde::Deserialize;
use serde_json::Value;

/// One fixture manifest entry shared by the Rust and Python V2 drift tests.
#[derive(Debug, Deserialize)]
struct FixtureEntry {
    /// Logical schema name used to select the parser.
    schema: String,
    /// Fixture filename relative to the worker-protocol fixture root.
    file: String,
}

/// Top-level manifest for the shared worker-protocol V2 fixtures.
#[derive(Debug, Deserialize)]
struct FixtureManifest {
    /// Canonical fixture entries consumed by both test suites.
    fixtures: Vec<FixtureEntry>,
}

/// Return the shared repo-level fixture directory for worker protocol V2.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/worker_protocol_v2")
}

/// Load and deserialize the shared fixture manifest.
fn load_manifest() -> FixtureManifest {
    let path = fixture_root().join("manifest.json");
    let raw = fs::read_to_string(path).expect("worker protocol v2 manifest should exist");
    serde_json::from_str(&raw).expect("worker protocol v2 manifest should parse")
}

/// Load one fixture file as raw JSON.
fn load_fixture_value(file: &str) -> Value {
    let path = fixture_root().join(file);
    let raw = fs::read_to_string(path).expect("worker protocol v2 fixture should exist");
    serde_json::from_str(&raw).expect("worker protocol v2 fixture should parse")
}

/// Parse and reserialize one fixture according to the declared schema.
fn roundtrip_fixture(schema: &str, raw: Value) -> Value {
    match schema {
        "hello_request" => serde_json::to_value(
            serde_json::from_value::<HelloRequestV2>(raw).expect("hello_request fixture is valid"),
        )
        .expect("hello_request should serialize"),
        "hello_response" => serde_json::to_value(
            serde_json::from_value::<HelloResponseV2>(raw)
                .expect("hello_response fixture is valid"),
        )
        .expect("hello_response should serialize"),
        "capabilities_request" => serde_json::to_value(
            serde_json::from_value::<CapabilitiesRequestV2>(raw)
                .expect("capabilities_request fixture is valid"),
        )
        .expect("capabilities_request should serialize"),
        "capabilities_response" => serde_json::to_value(
            serde_json::from_value::<CapabilitiesResponseV2>(raw)
                .expect("capabilities_response fixture is valid"),
        )
        .expect("capabilities_response should serialize"),
        "execute_request" => serde_json::to_value(
            serde_json::from_value::<ExecuteRequestV2>(raw)
                .expect("execute_request fixture is valid"),
        )
        .expect("execute_request should serialize"),
        "execute_response" => serde_json::to_value(
            serde_json::from_value::<ExecuteResponseV2>(raw)
                .expect("execute_response fixture is valid"),
        )
        .expect("execute_response should serialize"),
        "progress_event" => serde_json::to_value(
            serde_json::from_value::<ProgressEventV2>(raw)
                .expect("progress_event fixture is valid"),
        )
        .expect("progress_event should serialize"),
        "shutdown_request" => serde_json::to_value(
            serde_json::from_value::<ShutdownRequestV2>(raw)
                .expect("shutdown_request fixture is valid"),
        )
        .expect("shutdown_request should serialize"),
        other => panic!("unknown worker protocol v2 schema in manifest: {other}"),
    }
}

#[test]
fn worker_protocol_v2_fixtures_roundtrip_in_rust() {
    let manifest = load_manifest();
    assert!(
        !manifest.fixtures.is_empty(),
        "worker protocol v2 manifest should declare fixtures"
    );

    for entry in manifest.fixtures {
        let raw = load_fixture_value(&entry.file);
        let roundtripped = roundtrip_fixture(&entry.schema, raw.clone());
        assert_eq!(
            roundtripped, raw,
            "fixture {} should roundtrip through Rust schema {}",
            entry.file, entry.schema
        );
    }
}
