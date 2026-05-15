#!/usr/bin/env bash
# Build the static book HTML. Output: book/build/html/.
# $1 = mdbook binary (passed by sh_binary).
set -euo pipefail
MDBOOK="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
