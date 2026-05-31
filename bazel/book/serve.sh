#!/usr/bin/env bash
# Serve the book at http://localhost:3000 with auto-reload.
# $1 = mdbook binary (passed by sh_binary via @multitool//tools/mdbook).
# Falls back to PATH when the multitool-vendored binary is not reachable.
set -euo pipefail
MDBOOK="$1"; shift
if [[ ! -x "$MDBOOK" ]]; then
    for candidate in \
        "$(command -v mdbook 2>/dev/null || true)" \
        "$HOME/.cargo/bin/mdbook" \
        "/root/.cargo/bin/mdbook" \
        "/home/runner/.cargo/bin/mdbook" \
        "/opt/homebrew/bin/mdbook" \
        "/usr/local/bin/mdbook"; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then
            MDBOOK="$candidate"
            break
        fi
    done
fi
if [[ ! -x "$MDBOOK" ]]; then
    echo "serve.sh: mdbook not at '$MDBOOK' and not on PATH or known prefixes" >&2
    exit 1
fi
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" serve --open
