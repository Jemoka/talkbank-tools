//! Thin proxy for the workspace-wide wide-struct audit.
//!
//! `crates/xtask/src/wide_struct_audit.rs` owns the real scanning and allowance logic.
//! This test keeps that audit discoverable in `cargo nextest` output from the
//! root `talkbank-tools` package instead of duplicating identical wrappers under
//! multiple CLI crates.

use std::process::Command;

#[test]
fn wide_struct_audit_passes() {
    let status = Command::new("cargo")
        .args(["run", "-q", "-p", "xtask", "--", "lint-wide-structs"])
        .status()
        .expect("failed to run cargo run -q -p xtask -- lint-wide-structs");
    assert!(
        status.success(),
        "cargo run -q -p xtask -- lint-wide-structs failed"
    );
}
