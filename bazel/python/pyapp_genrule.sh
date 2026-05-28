#!/usr/bin/env bash
# Genrule wrapper around pyapp_build.sh.
#
# Why this exists: pyapp_build.sh requires BUILD_WORKSPACE_DIRECTORY
# (it cd's there + writes the wheel + writes the pyapp install root
# under python/target/...). That env var is only set by `bazel run`,
# not by `bazel build` actions. So invoking pyapp_build.sh directly
# from a genrule cmd fails.
#
# This wrapper runs inside a genrule action (with `local = True` so the
# host cargo + maturin escape paths still work). It:
#   1. Self-locates the workspace by following BASH_SOURCE[0] through
#      its symlink (Bazel symlinks srcs/tools from execroot back into
#      the source workspace), then walking up for MODULE.bazel.
#   2. Exports BUILD_WORKSPACE_DIRECTORY so the inner script behaves
#      identically to how it does under `bazel run`.
#   3. Forwards the remaining args to pyapp_build.sh.
#   4. Copies the escape-path output (python/target/pyapp/bin/sidecar)
#      to the genrule's declared $@ output path so Bazel can track it.
#
# Usage from a genrule:
#   $(execpath //bazel/python:pyapp_genrule.sh) $@ \
#       $(execpath @multitool//tools/uv) \
#       $(execpath //python/batchalign/_core:_proto_generated_py) \
#       opt \
#       $(execpath @llvm_toolchain_llvm//:bin/clang) \
#       $(execpath @llvm_toolchain_llvm//:bin/llvm-ar) \
#       $(execpath @llvm_toolchain_llvm//:bin/llvm-ranlib)

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "pyapp_genrule.sh: missing output path (first arg)" >&2
    exit 2
fi
# Bazel hands us a relative output path (under bazel-out/...). Resolve
# to absolute by joining with PWD; can't use `realpath -m` because
# macOS's stock realpath doesn't support `-m` (file may not yet exist).
case "$1" in
    /*) OUTPUT_PATH="$1" ;;
    *)  OUTPUT_PATH="$PWD/$1" ;;
esac
shift

# Resolve the workspace root from our own real path. Bazel staged us
# into execroot as a symlink pointing at workspace/bazel/python/
# pyapp_genrule.sh; realpath chases the symlink, dirname-walk-up finds
# MODULE.bazel.
self_real="$(realpath "${BASH_SOURCE[0]}")"
ws="$(dirname "$self_real")"
while [[ "$ws" != "/" && ! -f "$ws/MODULE.bazel" ]]; do
    ws="$(dirname "$ws")"
done
if [[ ! -f "$ws/MODULE.bazel" ]]; then
    echo "pyapp_genrule.sh: could not locate workspace MODULE.bazel from $self_real" >&2
    exit 2
fi
export BUILD_WORKSPACE_DIRECTORY="$ws"

# Hermetic rust toolchain. PYAPP_RUST_BIN_DIRS is set by the genrule's
# cmd to a space-separated list of execpaths into rules_rust's
# current_{cargo,rustc}_files filegroups. Pick out the absolute
# directory containing cargo and prepend it to PATH so the inner
# `cargo install pyapp` and `uv run maturin build` (which shells to
# cargo) use Bazel's pinned toolchain instead of host /usr/bin/cargo.
if [[ -n "${PYAPP_RUST_BIN_DIRS:-}" ]]; then
    for p in $PYAPP_RUST_BIN_DIRS; do
        case "$p" in
            */bin/cargo|*/bin/rustc)
                # Resolve to absolute, then take dirname.
                abs="$(cd "$(dirname "$p")" && pwd)"
                case ":$PATH:" in
                    *":$abs:"*) ;;
                    *) PATH="$abs:$PATH" ;;
                esac
                ;;
        esac
    done
    export PATH
    echo "pyapp_genrule.sh: prepended Bazel rust toolchain to PATH"
    echo "  cargo:  $(command -v cargo)"
    echo "  rustc:  $(command -v rustc)"
fi

echo "pyapp_genrule.sh: BUILD_WORKSPACE_DIRECTORY=$BUILD_WORKSPACE_DIRECTORY"
echo "pyapp_genrule.sh: invoking pyapp_build.sh with $# args"

# pyapp_build.sh writes the final binary to
# $BUILD_WORKSPACE_DIRECTORY/python/target/pyapp/bin/sidecar.
"$BUILD_WORKSPACE_DIRECTORY/bazel/python/pyapp_build.sh" "$@"

SIDECAR_BIN="$BUILD_WORKSPACE_DIRECTORY/python/target/pyapp/bin/sidecar"
if [[ ! -x "$SIDECAR_BIN" ]]; then
    echo "pyapp_genrule.sh: pyapp_build.sh did not produce $SIDECAR_BIN" >&2
    exit 2
fi

cp -f "$SIDECAR_BIN" "$OUTPUT_PATH"
chmod +x "$OUTPUT_PATH"
echo "pyapp_genrule.sh: copied to $OUTPUT_PATH"
