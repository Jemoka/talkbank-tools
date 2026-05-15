// Build scripts run at build time, not runtime. Panics here fail
// `cargo build`, which is the intended behaviour for missing files
// or invalid embedded data.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Build script — generates `BUILD_HASH` at compile time.
//!
//! The hash changes on every rebuild (even when `CARGO_PKG_VERSION` stays
//! the same), enabling stale-binary detection during development.

fn main() {
    let version = env!("CARGO_PKG_VERSION");

    // Best-effort git describe — empty string if git is not available.
    let git_describe = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let build_hash = if git_describe.is_empty() {
        format!("{version}-{epoch}")
    } else {
        format!("{version}-{git_describe}-{epoch}")
    };

    println!("cargo:rustc-env=BUILD_HASH={build_hash}");

    // Rebuild when git state changes (commit, stage, checkout).
    // These paths are relative to the manifest dir; we go up to the repo root.
    for path in ["../../../.git/HEAD", "../../../.git/index"] {
        println!("cargo:rerun-if-changed={path}");
    }
}
