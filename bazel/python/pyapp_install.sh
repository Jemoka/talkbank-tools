#!/usr/bin/env bash
# Build pyapp from local source via Bazel-provided cargo.
#
# Run as the cmd of //python/batchalign:sidecar (a genrule with
# `local = True`). Bazel hands us the Bazel-tracked wheel + the
# Bazel-fetched pyapp source as $(execpath …) args, plus PATH-injects
# rules_rust's cargo via PYAPP_RUST_BIN_DIRS. We invoke `cargo install
# --path <pyapp_src>` so cargo:
#   - never contacts crates.io (source is local)
#   - never uses host cargo (Bazel-provided cargo on PATH)
#   - creates its own writable tmpdir copy of the source (which pyapp's
#     build.rs needs — it writes to $CARGO_MANIFEST_DIR/src/embed/)
#
# `local = True` is required so cargo has a persistent place for its
# build cache; pure sandboxed actions would discard cargo's tmpdir
# between runs and pyapp's deps would re-compile every time.
#
# Args:
#   $1 = output binary path (the genrule's $@)
#   $2 = wheel path        (//python/batchalign:wheel_file)
#   $3 = pyapp Cargo.toml  (@pyapp_src//:Cargo.toml — we take dirname
#                           for `cargo install --path`)
#   $4 = compilation mode  (opt|dbg|fastbuild)

set -euo pipefail

if [[ $# -lt 4 ]]; then
    echo "pyapp_install.sh: usage: <output> <wheel> <pyapp_cargo_toml> <mode>" >&2
    exit 2
fi
case "$1" in /*) OUTPUT="$1" ;; *) OUTPUT="$PWD/$1" ;; esac
shift
WHEEL="$1"; shift
PYAPP_CARGO_TOML="$1"; shift
COMPILATION_MODE="$1"; shift

case "$WHEEL"            in /*) ;; *) WHEEL="$PWD/$WHEEL" ;; esac
case "$PYAPP_CARGO_TOML" in /*) ;; *) PYAPP_CARGO_TOML="$PWD/$PYAPP_CARGO_TOML" ;; esac
PYAPP_SRC_DIR="$(cd "$(dirname "$PYAPP_CARGO_TOML")" && pwd -P)"

[[ -f "$WHEEL" ]]            || { echo "wheel not found: $WHEEL" >&2; exit 2; }
[[ -d "$PYAPP_SRC_DIR" ]]    || { echo "pyapp source not found: $PYAPP_SRC_DIR" >&2; exit 2; }

# Self-locate workspace via realpath of BASH_SOURCE: Bazel symlinks
# this script into the action sandbox from its source location. cargo
# needs BUILD_WORKSPACE_DIRECTORY/python/target/... as a persistent
# cache root so deps aren't recompiled from scratch each run.
self_real="$(realpath "${BASH_SOURCE[0]}")"
ws="$(dirname "$self_real")"
while [[ "$ws" != "/" && ! -f "$ws/MODULE.bazel" ]]; do
    ws="$(dirname "$ws")"
done
[[ -f "$ws/MODULE.bazel" ]] || { echo "couldn't locate workspace MODULE.bazel" >&2; exit 2; }
export BUILD_WORKSPACE_DIRECTORY="$ws"

# Inject Bazel-provided cargo + rustc onto PATH so the inner cargo
# call never touches host cargo. PYAPP_RUST_BIN_DIRS is a space-
# separated list of $(execpaths …) for rules_rust's
# current_{cargo,rustc}_files filegroups.
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

out_dir="$BUILD_WORKSPACE_DIRECTORY/python/target/pyapp"
mkdir -p "$out_dir"

# Bazel materializes git_repository content read-only in its cache.
# pyapp's build.rs writes to $CARGO_MANIFEST_DIR/src/embed/ — an
# upstream design choice — so we need a writable copy of the source.
# `cargo install --path` (unlike `cargo install <pkg>` from registry)
# does NOT make its own tmpdir copy; it builds in-place at the
# manifest path. So we make our own writable copy here.
build_src="$BUILD_WORKSPACE_DIRECTORY/python/target/pyapp-src"
rm -rf "$build_src"
mkdir -p "$build_src"
cp -R "$PYAPP_SRC_DIR/." "$build_src/"
chmod -R u+w "$build_src"
PYAPP_SRC_DIR="$build_src"

# PyApp's build.rs parses the wheel filename per PEP 427
# (`{name}-{version}(-{build})?-{python}-{abi}-{platform}.whl`). The
# Bazel genrule output is a fixed `batchalign.whl` — Bazel doesn't
# permit dynamic output names — so we reconstruct the proper PEP-427
# filename here from the wheel's own `.dist-info/` metadata and stage
# a renamed copy. PyApp reads the contents identically; only the on-
# disk filename matters to its filename parser.
wheel_stage="$BUILD_WORKSPACE_DIRECTORY/python/target/pyapp-wheel"
rm -rf "$wheel_stage"
mkdir -p "$wheel_stage"
distinfo="$(unzip -l "$WHEEL" \
    | awk '{print $NF}' \
    | grep -oE '^[^/]+\.dist-info' \
    | head -1)"
if [[ -z "$distinfo" ]]; then
    echo "pyapp_install.sh: wheel has no .dist-info/ entry: $WHEEL" >&2
    exit 2
fi
name_version="${distinfo%.dist-info}"  # e.g. "batchalign-0.3.0"
tag="$(unzip -p "$WHEEL" "$distinfo/WHEEL" \
    | awk -F': ' '/^Tag:/ {print $2; exit}' \
    | tr -d '\r\n')"
if [[ -z "$tag" ]]; then
    echo "pyapp_install.sh: wheel has no Tag: in $distinfo/WHEEL" >&2
    exit 2
fi
real_wheel="$wheel_stage/${name_version}-${tag}.whl"
cp -f "$WHEEL" "$real_wheel"
echo "pyapp_install.sh: staged wheel = $real_wheel"
WHEEL="$real_wheel"

export PYAPP_PROJECT_PATH="$WHEEL"
export PYAPP_EXEC_SPEC="batchalign.cli.daemon:run_pyapp_entry"
export PYAPP_PYTHON_VERSION="3.12"
export PYAPP_DISTRIBUTION_EMBED="1"
export PYAPP_FULL_ISOLATION="1"
export PYAPP_PROJECT_FEATURES="api"

echo "pyapp_install.sh: cargo=$(command -v cargo)"
echo "pyapp_install.sh: pyapp source = $PYAPP_SRC_DIR"
echo "pyapp_install.sh: wheel = $WHEEL"

cargo install \
    --path "$PYAPP_SRC_DIR" \
    --force \
    --root "$out_dir" \
    "${cargo_flag[@]+"${cargo_flag[@]}"}"

cp -f "$out_dir/bin/pyapp" "$OUTPUT"
chmod +x "$OUTPUT"
echo "pyapp_install.sh: produced $OUTPUT"
