#!/usr/bin/env bash
# mdbook build + linkcheck (mdbook-linkcheck2 is wired via book.toml).
# $1 = mdbook binary (passed by sh_binary).
set -euo pipefail
MDBOOK="$1"; shift
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
