#!/usr/bin/env bash
# Regenerate the sqlx compile-time query cache for batchalign-cli.
# Commit the resulting .sqlx/ directory.
#
# sqlx-cli isn't published as a prebuilt binary by upstream, so it's
# installed once per developer via `cargo install sqlx-cli --features sqlite`.
# cargo comes from the rules_rust toolchain.
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY"
cargo sqlx prepare --manifest-path crates/batchalign/batchalign-cli/Cargo.toml -- --lib
