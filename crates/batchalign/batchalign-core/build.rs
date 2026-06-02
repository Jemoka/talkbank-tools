//! Bake the short git SHA into the crate as `VERGEN_GIT_SHA`.
//!
//! Used by `AsrTaskRunner` to stamp a `@Comment` provenance header onto every
//! ASR-generated CHAT file. Resolution order:
//!
//! 1. `BATCHALIGN_GIT_SHA` env var (set by CI / Bazel's
//!    `--workspace_status_command=bazel/stamp.sh`).
//! 2. `git rev-parse --short HEAD` in the source tree.
//! 3. `"unknown"` as a last resort.
//!
//! Mirrors `crates/batchalign/batchalign-engine/build.rs`; we resolve manually
//! to keep the build-deps surface small (no `vergen` pull).

use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BATCHALIGN_GIT_SHA");

    let sha = env::var("BATCHALIGN_GIT_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VERGEN_GIT_SHA={sha}");
}
