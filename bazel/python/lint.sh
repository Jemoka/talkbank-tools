#!/usr/bin/env bash
# mypy (+ ruff if installed) against the Python tree.
# $1 = uv binary (passed by sh_binary).
set -euo pipefail
UV="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/python"
"$UV" run mypy
if "$UV" run --quiet ruff --version >/dev/null 2>&1; then
    "$UV" run ruff check
fi
