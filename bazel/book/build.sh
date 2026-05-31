#!/usr/bin/env bash
# Build the static book HTML. Output: book/build/html/.
# $1 = mdbook binary (passed by sh_binary). Falls back to `mdbook` on PATH
# when the multitool-vendored binary isn't reachable (CI installs it via
# `cargo install mdbook` ahead of time; the bazel rule's runfiles
# resolution can race with rules_multitool's symlink layout).
set -euo pipefail
MDBOOK="$1"; shift
if [[ ! -x "$MDBOOK" ]]; then
    if command -v mdbook >/dev/null 2>&1; then
        MDBOOK="$(command -v mdbook)"
    else
        echo "build.sh: mdbook not at '$MDBOOK' and not on PATH" >&2
        exit 1
    fi
fi
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
