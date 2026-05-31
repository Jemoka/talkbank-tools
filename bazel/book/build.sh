#!/usr/bin/env bash
# Build the static book HTML. Output: book/build/html/.
# $1 = mdbook binary (passed by sh_binary). Falls back to `mdbook` on PATH
# when the multitool-vendored binary isn't reachable (CI installs it via
# `cargo install mdbook` ahead of time; the bazel rule's runfiles
# resolution can race with rules_multitool's symlink layout).
set -euo pipefail
MDBOOK_HINT="${1:-}"; shift || true

# Resolve mdbook. Priority order:
#   1. PATH (CI installs via `cargo install mdbook`; this is the most
#      reliable path because the workflow step runs before us).
#   2. Known cargo / homebrew install prefixes (sh_binary may have
#      stripped $PATH).
#   3. The multitool-vendored hint from `$(rootpath ...)`, resolved
#      against the workspace runfiles tree.
#   4. find(1) across the runfiles tree as a last resort.
resolve_mdbook() {
    local hint="$1"
    # 1. PATH
    if command -v mdbook >/dev/null 2>&1; then
        command -v mdbook
        return 0
    fi
    # 2. Cargo / homebrew prefixes
    for candidate in \
        "$HOME/.cargo/bin/mdbook" \
        "/root/.cargo/bin/mdbook" \
        "/home/runner/.cargo/bin/mdbook" \
        "/opt/homebrew/bin/mdbook" \
        "/usr/local/bin/mdbook"; do
        if [[ -x "$candidate" ]]; then echo "$candidate"; return 0; fi
    done
    # 3. Multitool-vendored hint (relative to runfiles roots).
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
    # 4. Glob.
    for root in "${RUNFILES_DIR:-}" "$PWD" "$PWD/.." "$PWD/../.."; do
        [[ -z "$root" || ! -d "$root" ]] && continue
        local hit
        hit="$(find "$root" -maxdepth 6 -path '*multitool*/tools/mdbook/mdbook' -type f 2>/dev/null | head -n 1)"
        if [[ -n "$hit" && -x "$hit" ]]; then echo "$hit"; return 0; fi
    done
    return 1
}

MDBOOK="$(resolve_mdbook "$MDBOOK_HINT")" || {
    echo "build.sh: mdbook not resolvable. HINT='$MDBOOK_HINT' RUNFILES_DIR='${RUNFILES_DIR:-}' PWD='$PWD' PATH='$PATH'" >&2
    exit 1
}
cd "$BUILD_WORKSPACE_DIRECTORY/book"
"$MDBOOK" build
