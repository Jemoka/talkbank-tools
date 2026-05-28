#!/usr/bin/env bash
# Resolve a `$(rootpath ...)` argument (as passed by sh_binary args) to
# an absolute filesystem path inside the runfiles tree.
#
# Bazel passes paths like:
#   external/toolchains_llvm++llvm+llvm_toolchain_llvm/bin/clang
#   python/batchalign/foo.py
# both of which need to be looked up relative to the runfiles directory
# the launcher prepared. RUNFILES_DIR is set by the rules_shell launcher;
# fall back to "$0.runfiles" for direct script invocations.
#
# Returns the resolved absolute path on stdout, or exits non-zero with a
# diagnostic if the file is not findable.
runfiles_resolve() {
    local rloc="$1"
    local runfiles_dir="${RUNFILES_DIR:-${BASH_SOURCE[0]}.runfiles}"
    if [[ -z "$rloc" ]]; then
        echo "runfiles_resolve: empty path argument" >&2
        return 2
    fi
    # Try the main-repo layout (`_main/<path>`), then the legacy/external
    # layout (`<path>` directly under runfiles dir), then absolute paths
    # (some `$(rootpath)` invocations resolve to absolute paths under
    # bazel-bin already).
    local candidates=(
        "$runfiles_dir/_main/$rloc"
        "$runfiles_dir/$rloc"
        "$rloc"
    )
    for cand in "${candidates[@]}"; do
        if [[ -f "$cand" ]]; then
            (cd "$(dirname "$cand")" && printf '%s/%s\n' "$(pwd)" "$(basename "$cand")")
            return 0
        fi
    done
    echo "runfiles_resolve: cannot locate '$rloc' (tried: ${candidates[*]})" >&2
    return 2
}
