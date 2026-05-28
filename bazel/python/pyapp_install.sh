#!/usr/bin/env bash
# Install pyapp with an already-built wheel — no maturin step.
#
# Split out of pyapp_build.sh to remove the duplicate maturin build:
# the sidecar genrule consumes :wheel_file (which already produced a
# Bazel-tracked .whl via maturin_genrule.sh) and passes that wheel
# path here. This script ONLY does Step 2 of the old pyapp_build.sh
# (cargo install pyapp with PYAPP_* env vars).
#
# Remaining escape: `cargo install pyapp --version` reaches out to
# crates.io to fetch pyapp's source. To eliminate this last escape,
# pin pyapp via rules_rust crate_universe's bindep pattern:
#   crate.spec(package = "pyapp", version = "=0.27.0", artifact = "bin")
#   crate.annotation(crate = "pyapp", gen_all_binaries = True)
# Then the sidecar genrule consumes @crates//:pyapp__bin directly —
# zero network access at build time. (See user feedback in
# bazel-reactive-builds.md.) Not done in this pass because the
# PYAPP_* env vars are baked at compile time and require careful
# action-time injection; tracked as follow-up.
#
# Args:
#   $1 = wheel path (Bazel-tracked artifact from :wheel_file)
#   $2 = output binary path (caller hands us where to put the result)
#   $3 = compilation mode (opt|dbg|fastbuild)

set -euo pipefail

if [[ $# -lt 3 ]]; then
    echo "pyapp_install.sh: usage: <wheel> <output> <mode>" >&2
    exit 2
fi
WHEEL="$1"; shift
OUTPUT="$1"; shift
COMPILATION_MODE="${1:-opt}"; shift

# Resolve absolute paths (cargo install --root needs absolute).
case "$WHEEL" in /*) ;; *) WHEEL="$PWD/$WHEEL" ;; esac
case "$OUTPUT" in /*) ;; *) OUTPUT="$PWD/$OUTPUT" ;; esac
if [[ ! -f "$WHEEL" ]]; then
    echo "pyapp_install.sh: wheel not found: $WHEEL" >&2
    exit 2
fi

# Read pinned pyapp + python versions from pyproject.toml (same source
# of truth pyapp_build.sh used). Workspace path is required for this;
# callers must export BUILD_WORKSPACE_DIRECTORY.
pyproject="${BUILD_WORKSPACE_DIRECTORY:?BUILD_WORKSPACE_DIRECTORY must be set}/python/pyproject.toml"
pin_pyapp=$(sed -n "/^\[tool\.batchalign\.pinned_tools\]/,/^\[/p" "$pyproject" \
    | sed -n 's/^pyapp[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
pin_python=$(sed -n "/^\[tool\.batchalign\.pinned_tools\]/,/^\[/p" "$pyproject" \
    | sed -n 's/^python[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
if [[ -z "$pin_pyapp" ]]; then
    echo "pyapp_install.sh: missing pyapp pin in [tool.batchalign.pinned_tools]" >&2
    exit 2
fi

case "$COMPILATION_MODE" in
    release|opt) cargo_flag=() ;;
    debug|dbg|fastbuild) cargo_flag=(--debug) ;;
    *) echo "pyapp_install.sh: unknown profile $COMPILATION_MODE" >&2; exit 2 ;;
esac

# Use a workspace-anchored tempdir so cargo's build cache survives
# between invocations (cargo install reuses target/ artifacts when the
# same `--root` is given again).
out_dir="${BUILD_WORKSPACE_DIRECTORY}/python/target/pyapp"
mkdir -p "$out_dir"

export PYAPP_PROJECT_PATH="$WHEEL"
export PYAPP_EXEC_SPEC="batchalign.cli.daemon:run_pyapp_entry"
export PYAPP_PYTHON_VERSION="${PYAPP_PYTHON_VERSION:-$pin_python}"
export PYAPP_DISTRIBUTION_EMBED="${PYAPP_DISTRIBUTION_EMBED:-1}"
export PYAPP_FULL_ISOLATION="${PYAPP_FULL_ISOLATION:-1}"
export PYAPP_PROJECT_DEPENDENCY_FILE=""
export PYAPP_PIP_EXTRA_ARGS="${PYAPP_PIP_EXTRA_ARGS:-}"
export PYAPP_PROJECT_FEATURES="api"

echo "pyapp_install.sh: cargo install pyapp@$pin_pyapp"
echo "  PYAPP_PROJECT_PATH=$PYAPP_PROJECT_PATH"
echo "  PYAPP_PYTHON_VERSION=$PYAPP_PYTHON_VERSION"

cargo install pyapp \
    --version "$pin_pyapp" \
    --force \
    --root "$out_dir" \
    "${cargo_flag[@]+"${cargo_flag[@]}"}"

cp -f "$out_dir/bin/pyapp" "$OUTPUT"
chmod +x "$OUTPUT"
echo "pyapp_install.sh: produced $OUTPUT"
