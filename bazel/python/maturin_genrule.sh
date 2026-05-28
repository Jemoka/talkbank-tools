#!/usr/bin/env bash
# Genrule wrapper around maturin_build.sh.
#
# Mirrors pyapp_genrule.sh's pattern: self-locate the workspace (so
# maturin_build.sh's BUILD_WORKSPACE_DIRECTORY contract works inside a
# `bazel build` action), then forward args, then capture the produced
# wheel as the genrule's tracked output.
#
# maturin_build.sh writes the wheel to
# $BUILD_WORKSPACE_DIRECTORY/python/target/wheels/batchalign-*.whl.
# We glob for that pattern + cp into $@.
#
# Usage from a genrule:
#   $(execpath //bazel/python:maturin_genrule.sh) $@ \
#       $(execpath @multitool//tools/uv) \
#       $(execpath //python/batchalign/_core:_proto_generated_py) \
#       opt \
#       $(execpath @llvm_toolchain_llvm//:bin/clang) \
#       $(execpath @llvm_toolchain_llvm//:bin/llvm-ar) \
#       $(execpath @llvm_toolchain_llvm//:bin/llvm-ranlib)

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "maturin_genrule.sh: missing output path (first arg)" >&2
    exit 2
fi
case "$1" in
    /*) OUTPUT_PATH="$1" ;;
    *)  OUTPUT_PATH="$PWD/$1" ;;
esac
shift

self_real="$(realpath "${BASH_SOURCE[0]}")"
ws="$(dirname "$self_real")"
while [[ "$ws" != "/" && ! -f "$ws/MODULE.bazel" ]]; do
    ws="$(dirname "$ws")"
done
if [[ ! -f "$ws/MODULE.bazel" ]]; then
    echo "maturin_genrule.sh: could not locate workspace MODULE.bazel from $self_real" >&2
    exit 2
fi
export BUILD_WORKSPACE_DIRECTORY="$ws"

# Inject Bazel-provided rust toolchain (same as pyapp_genrule.sh).
if [[ -n "${PYAPP_RUST_BIN_DIRS:-}" ]]; then
    for p in $PYAPP_RUST_BIN_DIRS; do
        case "$p" in
            */bin/cargo|*/bin/rustc)
                abs="$(cd "$(dirname "$p")" && pwd)"
                case ":$PATH:" in
                    *":$abs:"*) ;;
                    *) PATH="$abs:$PATH" ;;
                esac
                ;;
        esac
    done
    export PATH
fi

echo "maturin_genrule.sh: BUILD_WORKSPACE_DIRECTORY=$BUILD_WORKSPACE_DIRECTORY"
"$BUILD_WORKSPACE_DIRECTORY/bazel/python/maturin_build.sh" "$@"

# maturin writes to target/wheels/batchalign-*.whl. Glob for the
# freshest one and copy into $@.
wheel="$(ls -t "$BUILD_WORKSPACE_DIRECTORY/python/target/wheels/batchalign-"*.whl 2>/dev/null | head -1 || true)"
if [[ -z "$wheel" || ! -f "$wheel" ]]; then
    echo "maturin_genrule.sh: maturin did not produce a wheel" >&2
    exit 2
fi
cp -f "$wheel" "$OUTPUT_PATH"
echo "maturin_genrule.sh: copied $wheel → $OUTPUT_PATH"
