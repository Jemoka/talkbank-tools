#!/usr/bin/env bash
# Genrule wrapper around maturin_build.sh.
#
# Mirrors pyapp_genrule.sh's pattern: self-locate the workspace (so
# maturin_build.sh's BUILD_WORKSPACE_DIRECTORY contract works inside a
# `bazel build` action), then forward args, then capture the produced
# wheel + its maturin-assigned tagged filename as the genrule's two
# tracked outputs.
#
# maturin_build.sh writes the wheel to
# $BUILD_WORKSPACE_DIRECTORY/python/target/wheels/batchalign-*.whl.
# We glob for the freshest match, cp the bytes into OUT_WHEEL, and
# write the basename (e.g. `batchalign-0.7.1-cp312-cp312-macosx_11_0_arm64.whl`)
# into OUT_TAG. Downstream targets that need the PyPI-tagged name
# (publish recipe, `just batchalign wheel`'s copy-to-source-tree)
# read OUT_TAG; downstream targets that just want the wheel bytes
# (sidecar PyApp build) read OUT_WHEEL directly.
#
# Usage from a genrule:
#   $(execpath //bazel/python:maturin_genrule.sh) \
#       $(location batchalign.whl) \
#       $(location wheel_tag.txt) \
#       $(execpath @multitool//tools/uv) \
#       $(execpath //python/batchalign/_core:_proto_generated_py) \
#       opt \
#       llvm \
#       $(execpath //bazel/python:llvm_clang) \
#       $(execpath //bazel/python:llvm_ar) \
#       $(execpath //bazel/python:llvm_ranlib)

set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "maturin_genrule.sh: usage: <out-wheel> <out-tag> <maturin-args...>" >&2
    exit 2
fi
case "$1" in
    /*) OUT_WHEEL="$1" ;;
    *)  OUT_WHEEL="$PWD/$1" ;;
esac
shift
case "$1" in
    /*) OUT_TAG="$1" ;;
    *)  OUT_TAG="$PWD/$1" ;;
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
            */bin/cargo|*/bin/cargo.exe|*/bin/rustc|*/bin/rustc.exe)
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
# freshest one, copy bytes into OUT_WHEEL, and record the maturin-
# assigned tagged filename in OUT_TAG so downstream targets that need
# the PyPI-tagged name (publish / `just batchalign wheel`) read it
# from a Bazel-tracked file rather than re-deriving it.
wheel="$(ls -t "$BUILD_WORKSPACE_DIRECTORY/python/target/wheels/batchalign-"*.whl 2>/dev/null | head -1 || true)"
if [[ -z "$wheel" || ! -f "$wheel" ]]; then
    echo "maturin_genrule.sh: maturin did not produce a wheel" >&2
    exit 2
fi
cp -f "$wheel" "$OUT_WHEEL"
basename "$wheel" > "$OUT_TAG"
echo "maturin_genrule.sh: copied $wheel → $OUT_WHEEL (tag $(cat "$OUT_TAG"))"
