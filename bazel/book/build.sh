#!/usr/bin/env bash
# Build the static book HTML. Output: book/build/html/.
# $1 = mdbook binary (passed by sh_binary). Falls back to `mdbook` on PATH
# when the multitool-vendored binary isn't reachable (CI installs it via
# `cargo install mdbook` ahead of time; the bazel rule's runfiles
# resolution can race with rules_multitool's symlink layout).
set -euo pipefail
MDBOOK="$1"; shift
# Probe common locations when the bazel-runfiles path isn't reachable.
# CI installs mdbook via `cargo install mdbook` (lands in ~/.cargo/bin),
# but sh_binary doesn't propagate the developer's $PATH. Search PATH +
# known cargo / homebrew prefixes before giving up.
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
    echo "build.sh: mdbook not at '$MDBOOK' and not on PATH or known prefixes" >&2
    exit 1
fi
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
