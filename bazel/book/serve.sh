#!/usr/bin/env bash
# Serve the book at http://localhost:3000 with auto-reload.
# $1 = mdbook binary (passed by sh_binary via @multitool//tools/mdbook).
set -euo pipefail
MDBOOK="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" serve --open
