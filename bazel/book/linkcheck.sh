#!/usr/bin/env bash
# mdbook build + linkcheck (mdbook-linkcheck2 is wired via book.toml).
# $1 = mdbook binary (passed by sh_binary). Falls back to PATH when the
# multitool-vendored binary is not reachable.
set -euo pipefail
MDBOOK="$1"; shift

# See bazel/book/build.sh for the resolution rationale.
resolve_mdbook() {
    local p="$1"
    [[ -x "$p" ]] && { echo "$p"; return; }
    for root in "${RUNFILES_DIR:-}" "${TEST_SRCDIR:-}" "${PWD}" "${PWD}/.."; do
        [[ -z "$root" ]] && continue
        if [[ -x "$root/$p" ]]; then echo "$root/$p"; return; fi
        local stripped="${p#../}"
        if [[ "$stripped" != "$p" && -x "$root/$stripped" ]]; then
            echo "$root/$stripped"; return
        fi
    done
    for root in "${RUNFILES_DIR:-}" "${PWD}" "${PWD}/.." "${PWD}/../.."; do
        [[ -z "$root" || ! -d "$root" ]] && continue
        local hit
        hit="$(find "$root" -maxdepth 6 -path '*multitool*/tools/mdbook/mdbook' -type f -perm -u+x 2>/dev/null | head -n 1)"
        if [[ -n "$hit" ]]; then echo "$hit"; return; fi
    done
    for candidate in \
        "$(command -v mdbook 2>/dev/null || true)" \
        "$HOME/.cargo/bin/mdbook" \
        "/root/.cargo/bin/mdbook" \
        "/home/runner/.cargo/bin/mdbook" \
        "/opt/homebrew/bin/mdbook" \
        "/usr/local/bin/mdbook"; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then echo "$candidate"; return; fi
    done
    return 1
}
RESOLVED="$(resolve_mdbook "$MDBOOK")" || {
    echo "linkcheck.sh: mdbook not resolvable from '$MDBOOK', RUNFILES_DIR='${RUNFILES_DIR:-}', PWD='$PWD'" >&2
    exit 1
}
MDBOOK="$RESOLVED"
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
