#!/usr/bin/env bash
# Build the batchalign3 wheel. Output: python/target/wheels/.
# $1 = uv binary (passed by sh_binary via @multitool//tools/uv).
set -euo pipefail
UV="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/python"
profile="${MATURIN_PROFILE:-release}"
"$UV" run maturin build --"${profile}" \
    --manifest-path ../crates/batchalign/batchalign-pyo3/Cargo.toml \
    --out target/wheels \
    "$@"
ls -lh target/wheels/ || true
