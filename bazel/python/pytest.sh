#!/usr/bin/env bash
# pytest against the editable batchalign install.
# Requires `bazel run //python/batchalign:develop` first.
#
# Hermeticity guarded before invocation.
#
# $1 = uv binary (passed by sh_binary).
set -euo pipefail
UV="$1"; shift

# shellcheck source=hermeticity_guard.sh
source "$(dirname "${BASH_SOURCE[0]}")/hermeticity_guard.sh"
hermeticity_guard "$UV"

cd "$BUILD_WORKSPACE_DIRECTORY/python"
"$UV" run pytest "$@"
