#!/usr/bin/env bash
# Check that Rust IPC types and generated JSON Schema are in sync.
#
# Exits non-zero if any schema files are stale or missing.
# Run after modifying Rust types that cross the Python boundary.
#
# Usage: bash scripts/check_ipc_type_drift.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo run -q -p batchalign-cli -- ipc-schema --check --output schemas/ipc/
