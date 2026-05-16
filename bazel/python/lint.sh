#!/usr/bin/env bash
# mypy (+ ruff if installed) against the Python tree.
#
# Hermeticity guarded before invocation.
#
# $1 = uv binary (passed by sh_binary).
set -euo pipefail
UV="$1"; shift

# shellcheck source=hermeticity_guard.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/hermeticity_guard.sh"
hermeticity_guard "$UV"

cd "$BUILD_WORKSPACE_DIRECTORY/python"
"$UV" run mypy
if "$UV" run --quiet ruff --version >/dev/null 2>&1; then
    "$UV" run ruff check
fi
