#!/usr/bin/env bash
# Repin the crate_universe lockfile after editing any Cargo.toml.
# bazelisk is the entry point; cargo comes from the rules_rust toolchain
# at sync time.
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY"
CARGO_BAZEL_REPIN=true bazel sync --only=crates
