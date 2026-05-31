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
# the generator writes back into the source tree (and needs to import
# `batchalign` from the workspace's uv-managed venv).
#
# - `bazel run`  sets BUILD_WORKSPACE_DIRECTORY.
# - `bazel test` does NOT, so walk upward from $PWD looking for the
#   MODULE.bazel marker.
find_workspace() {
    local dir="$PWD"
    while [[ "$dir" != "/" ]]; do
        if [[ -f "$dir/MODULE.bazel" ]]; then
            echo "$dir"
            return
        fi
        dir="$(dirname "$dir")"
    done
    echo "$PWD"
}
ROOT="${BUILD_WORKSPACE_DIRECTORY:-$(find_workspace)}"
cd "$ROOT"

# Prefer the workspace's uv-managed venv (python/.venv) since `bazel
# test` runs in a sandbox where the system python has no batchalign.
# Honor an explicit PYTHON override, then probe known venvs, then fall
# back to `python3`.
pick_python() {
    local candidate
    if [[ -n "${PYTHON:-}" ]]; then
        echo "$PYTHON"
        return
    fi
    for candidate in \
        "$ROOT/python/.venv/bin/python" \
        "$ROOT/.venv/bin/python" \
        "$(command -v python3 || true)"; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then
            if "$candidate" -c "import batchalign, fastapi" 2>/dev/null; then
                echo "$candidate"
                return
            fi
        fi
    done
    echo "${PYTHON:-python3}"
}

PYTHON="$(pick_python)"

# Ensure `node` is on PATH so `openapi-typescript` (a #!/usr/bin/env node
# script) resolves. Bazel's `local` sandbox strips PATH down to a tiny
# set of dirs; probe common Homebrew install locations and prepend.
ensure_node() {
    if command -v node >/dev/null 2>&1; then
        return
    fi
    local candidate
    for candidate in \
        "/opt/homebrew/opt/node@22/bin" \
        "/opt/homebrew/opt/node@20/bin" \
        "/opt/homebrew/bin" \
        "/usr/local/bin"; do
        if [[ -x "$candidate/node" ]]; then
            export PATH="$candidate:$PATH"
            return
        fi
    done
}
ensure_node

exec "$PYTHON" scripts/gen_openapi.py "$@"
