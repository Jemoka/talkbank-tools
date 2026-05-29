#!/usr/bin/env bash
# Build cargo-tauri from local source via Bazel-provided cargo.
#
# Run as the cmd of //bazel/tauri:cargo_tauri (a genrule with
# `local = True`). Bazel hands us the http_archive-fetched tauri-cli
# source via $(execpath …) and PATH-injects rules_rust's cargo via
# PYAPP_RUST_BIN_DIRS. We invoke `cargo install --path <tauri_src>`
# with Bazel cargo so:
#   - no host cargo dependency
#   - tauri-cli source pinned by SHA via http_archive (no live
#     crates.io fetch for the root crate; transitive deps still resolve
#     via cargo's normal mechanisms — same scope as pyapp's :sidecar)
#
# Args:
#   $1 = output binary path (the genrule's $@)
#   $2 = tauri-cli Cargo.toml (@tauri_cli_src//:Cargo.toml — we take
#        dirname to get the source-tree root)
#   $3 = compilation mode (opt|dbg|fastbuild)

set -euo pipefail

if [[ $# -lt 3 ]]; then
    echo "tauri_install.sh: usage: <output> <cargo_toml> <mode>" >&2
    exit 2
fi
case "$1" in /*) OUTPUT="$1" ;; *) OUTPUT="$PWD/$1" ;; esac
shift
CARGO_TOML="$1"; shift
COMPILATION_MODE="${1:-opt}"; shift

case "$CARGO_TOML" in /*) ;; *) CARGO_TOML="$PWD/$CARGO_TOML" ;; esac
SRC_DIR="$(cd "$(dirname "$CARGO_TOML")" && pwd -P)"

[[ -d "$SRC_DIR" ]] || { echo "tauri source not found: $SRC_DIR" >&2; exit 2; }

self_real="$(realpath "${BASH_SOURCE[0]}")"
ws="$(dirname "$self_real")"
while [[ "$ws" != "/" && ! -f "$ws/MODULE.bazel" ]]; do
    ws="$(dirname "$ws")"
done
[[ -f "$ws/MODULE.bazel" ]] || { echo "couldn't locate workspace MODULE.bazel" >&2; exit 2; }
export BUILD_WORKSPACE_DIRECTORY="$ws"

# PATH-inject Bazel-provided cargo + rustc.
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

case "$COMPILATION_MODE" in
    release|opt)         cargo_flag=() ;;
    debug|dbg|fastbuild) cargo_flag=(--debug) ;;
    *) echo "unknown profile $COMPILATION_MODE" >&2; exit 2 ;;
esac

# Bazel http_archive content is read-only; cargo install --path builds
# in-place. Copy to a writable workspace dir so cargo can do its work
# (matches the pyapp_install.sh approach).
build_src="$BUILD_WORKSPACE_DIRECTORY/python/target/tauri-cli-src"
rm -rf "$build_src"
mkdir -p "$build_src"
cp -R "$SRC_DIR/." "$build_src/"
chmod -R u+w "$build_src"

out_dir="$BUILD_WORKSPACE_DIRECTORY/python/target/tauri-cli"
mkdir -p "$out_dir"

echo "tauri_install.sh: cargo=$(command -v cargo)"
echo "tauri_install.sh: tauri-cli source = $build_src"

cargo install \
    --path "$build_src" \
    --force \
    --root "$out_dir" \
    "${cargo_flag[@]+"${cargo_flag[@]}"}"

# cargo install of `tauri-cli` produces a binary called `cargo-tauri`.
cp -f "$out_dir/bin/cargo-tauri" "$OUTPUT"
chmod +x "$OUTPUT"
echo "tauri_install.sh: produced $OUTPUT"
