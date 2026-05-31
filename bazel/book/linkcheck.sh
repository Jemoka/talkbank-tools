#!/usr/bin/env bash
# mdbook build + linkcheck (mdbook-linkcheck2 is wired via book.toml).
# $1 = mdbook binary (passed by sh_binary). Falls back to PATH when the
# multitool-vendored binary is not reachable.
set -euo pipefail
MDBOOK_HINT="${1:-}"; shift || true

resolve_mdbook() {
    local hint="$1"
    if command -v mdbook >/dev/null 2>&1; then
        command -v mdbook; return 0
    fi
    for candidate in \
        "$HOME/.cargo/bin/mdbook" \
        "/root/.cargo/bin/mdbook" \
        "/home/runner/.cargo/bin/mdbook" \
        "/opt/homebrew/bin/mdbook" \
        "/usr/local/bin/mdbook"; do
        if [[ -x "$candidate" ]]; then echo "$candidate"; return 0; fi
    done
    if [[ -n "$hint" ]]; then
        if [[ -x "$hint" ]]; then echo "$hint"; return 0; fi
        local stripped="${hint#../}"
        for root in "${RUNFILES_DIR:-}" "${TEST_SRCDIR:-}" "$PWD" "$PWD/.."; do
            [[ -z "$root" ]] && continue
            if [[ -x "$root/$hint" ]]; then echo "$root/$hint"; return 0; fi
            if [[ "$stripped" != "$hint" && -x "$root/$stripped" ]]; then
                echo "$root/$stripped"; return 0
            fi
        done
    fi
    for root in "${RUNFILES_DIR:-}" "$PWD" "$PWD/.." "$PWD/../.."; do
        [[ -z "$root" || ! -d "$root" ]] && continue
        local hit
        hit="$(find "$root" -maxdepth 6 -path '*multitool*/tools/mdbook/mdbook' -type f 2>/dev/null | head -n 1)"
        if [[ -n "$hit" && -x "$hit" ]]; then echo "$hit"; return 0; fi
    done
    return 1
}

MDBOOK="$(resolve_mdbook "$MDBOOK_HINT")" || {
    echo "linkcheck.sh: mdbook not resolvable. HINT='$MDBOOK_HINT' RUNFILES_DIR='${RUNFILES_DIR:-}' PWD='$PWD' PATH='$PATH'" >&2
    exit 1
}
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
