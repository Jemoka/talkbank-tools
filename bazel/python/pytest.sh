#!/usr/bin/env bash
# pytest against the editable batchalign3 install.
# Requires `bazel run //python/batchalign:develop` first.
# $1 = uv binary (passed by sh_binary).
set -euo pipefail
UV="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/python"
"$UV" run pytest "$@"
