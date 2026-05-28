#!/usr/bin/env bash
# Genrule wrapper around pyapp_install.sh.
#
# Consumes a pre-built wheel (from :wheel_file's genrule, the
# canonical Bazel-tracked .whl artifact) and runs ONLY the
# cargo-install-pyapp step. The wheel is NOT rebuilt here — the
# previous version of this script duplicated maturin work that
# :wheel_file already does.
#
# Self-locates the workspace via realpath-walk so
# BUILD_WORKSPACE_DIRECTORY is available to pyapp_install.sh.
#
# Args:
#   $1 = output binary path (the genrule's $@)
#   $2 = wheel path (the :wheel_file output, Bazel-tracked)
#   $3 = compilation mode (opt|dbg|fastbuild)

set -euo pipefail

if [[ $# -lt 3 ]]; then
    echo "pyapp_genrule.sh: usage: <output> <wheel> <mode>" >&2
    exit 2
fi
case "$1" in /*) OUTPUT_PATH="$1" ;; *) OUTPUT_PATH="$PWD/$1" ;; esac
shift
WHEEL="$1"; shift
COMPILATION_MODE="$1"; shift

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

# Hermetic rust toolchain — see comment in pyapp_install.sh for the
# remaining cargo-install-pyapp escape that this PATH injection
# narrows but doesn't eliminate.
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

echo "pyapp_genrule.sh: BUILD_WORKSPACE_DIRECTORY=$BUILD_WORKSPACE_DIRECTORY"
"$BUILD_WORKSPACE_DIRECTORY/bazel/python/pyapp_install.sh" "$WHEEL" "$OUTPUT_PATH" "$COMPILATION_MODE"
