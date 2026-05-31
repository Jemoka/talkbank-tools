#!/usr/bin/env bash
# mdbook build + linkcheck (mdbook-linkcheck2 is wired via book.toml).
# $1 = mdbook binary (passed by sh_binary). Falls back to PATH when the
# multitool-vendored binary is not reachable.
set -euo pipefail
MDBOOK="$1"; shift
if [[ ! -x "$MDBOOK" ]]; then
    if command -v mdbook >/dev/null 2>&1; then
        MDBOOK="$(command -v mdbook)"
    else
        echo "linkcheck.sh: mdbook not at '$MDBOOK' and not on PATH" >&2
        exit 1
    fi
fi
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
