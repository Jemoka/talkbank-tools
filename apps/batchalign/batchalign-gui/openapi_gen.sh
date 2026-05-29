#!/usr/bin/env bash
# Shim that invokes scripts/gen_openapi.py against the live workspace.
# Forwards args (e.g. `--check` for freshness mode) verbatim.
#
# This shell shim exists because the codegen needs to import
# `batchalign.api` (a Python package whose runtime resolution is host-
# venv dependent), which doesn't fit cleanly into a Bazel py_binary
# until we wire up `aspect_rules_py` for this corner. The script
# lives in `scripts/` (Bazel-stable home) and the GUI's BUILD.bazel
# exposes the user-facing `:openapi` and `:openapi_freshness` names
# that point at it through this shim.
set -euo pipefail

# Anchor on the source workspace, not the Bazel sandbox runfiles, since
# the generator writes back into the source tree.
ROOT="${BUILD_WORKSPACE_DIRECTORY:-$PWD}"
cd "$ROOT"

# Prefer the host python in the active venv (uv-managed); fall back to
# `python3`. Either must have batchalign + fastapi importable.
PYTHON="${PYTHON:-python3}"
exec "$PYTHON" scripts/gen_openapi.py "$@"
