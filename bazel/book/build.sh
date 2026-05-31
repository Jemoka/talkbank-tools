#!/usr/bin/env bash
# Build the static book HTML. Output: book/build/html/.
# $1 = mdbook binary (passed by sh_binary). Falls back to `mdbook` on PATH
# when the multitool-vendored binary isn't reachable (CI installs it via
# `cargo install mdbook` ahead of time; the bazel rule's runfiles
# resolution can race with rules_multitool's symlink layout).
set -euo pipefail
MDBOOK="$1"; shift

# Resolve the multitool-vendored mdbook. sh_binary passes the runfiles
# rootpath which is relative to the workspace runfiles dir; --legacy_
# external_runfiles=false puts the external repo at runfiles ROOT, not
# nested under <workspace>/external/. The path may need to be resolved
# via RUNFILES_DIR / TEST_SRCDIR / by walking the runfiles tree.
resolve_mdbook() {
    local p="$1"
    # Direct hit.
    [[ -x "$p" ]] && { echo "$p"; return; }
    # Resolve relative to known runfiles roots.
    for root in \
        "${RUNFILES_DIR:-}" \
        "${TEST_SRCDIR:-}" \
        "${PWD}" \
        "${PWD}/.."; do
        [[ -z "$root" ]] && continue
        if [[ -x "$root/$p" ]]; then echo "$root/$p"; return; fi
        # Strip leading ../ and try again.
        local stripped="${p#../}"
        if [[ "$stripped" != "$p" && -x "$root/$stripped" ]]; then
            echo "$root/$stripped"; return
        fi
    done
    # Last resort: glob across the runfiles for the binary.
    for root in "${RUNFILES_DIR:-}" "${PWD}" "${PWD}/.." "${PWD}/../.."; do
        [[ -z "$root" || ! -d "$root" ]] && continue
        local hit
        hit="$(find "$root" -maxdepth 6 -path '*multitool*/tools/mdbook/mdbook' -type f -perm -u+x 2>/dev/null | head -n 1)"
        if [[ -n "$hit" ]]; then echo "$hit"; return; fi
    done
    # PATH + cargo / homebrew prefixes (CI may have done `cargo install
    # mdbook` ahead of time).
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
    echo "build.sh: mdbook not resolvable from '$MDBOOK', RUNFILES_DIR='${RUNFILES_DIR:-}', PWD='$PWD'" >&2
    exit 1
}
MDBOOK="$RESOLVED"
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
