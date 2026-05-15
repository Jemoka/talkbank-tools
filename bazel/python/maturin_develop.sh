#!/usr/bin/env bash
# Editable install of the batchalign3 wheel.
# $1 = uv binary (passed by sh_binary via @multitool//tools/uv).
# maturin lives inside the uv-managed venv; `uv run` finds it.
set -euo pipefail
UV="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/python"
"$UV" run maturin develop \
    --manifest-path ../crates/batchalign/batchalign-pyo3/Cargo.toml \
    "$@"
