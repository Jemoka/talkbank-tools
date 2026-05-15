//! Thin proxy — delegates to `cargo run -q -p xtask -- lint-docs-sync`.
//!
//! The actual docs sync checks live in `crates/xtask/src/docs_sync.rs` to avoid
//! compiling a full integration test binary just for a structural lint.

use std::process::Command;

#[test]
fn docs_sync_passes() {
    let status = Command::new("cargo")
        .args(["run", "-q", "-p", "xtask", "--", "lint-docs-sync"])
        .status()
        .expect("failed to run cargo xtask lint-docs-sync");
    assert!(status.success(), "cargo xtask lint-docs-sync failed");
}
