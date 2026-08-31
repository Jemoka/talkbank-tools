#!/usr/bin/env bash
# Copy the Bazel-built wheel into the source-tree wheel directory under
# its maturin-assigned PyPI-tagged basename.
#
# Run as the entry point of `bazel run //python/batchalign:wheel`. The
# heavy work — invoking maturin, building the .so, packaging the wheel
# — happens once inside the `:_wheel_artifact` genrule and is
# Bazel-cached; this script is just the final copy-out so contributors,
# the justfile, and CI all have one canonical command:
#
#     bazel run //python/batchalign:wheel
#
# Args (passed by sh_binary `args` as rootpaths):
#   $1 = path to batchalign.whl    (//python/batchalign:batchalign.whl)
#   $2 = path to wheel_tag.txt     (//python/batchalign:wheel_tag.txt)
#
# Writes to:
#   $BUILD_WORKSPACE_DIRECTORY/python/target/wheels/<tag>.whl
#   where <tag> is the basename maturin assigned, e.g.
#   `batchalign-0.7.1-cp312-cp312-macosx_11_0_arm64.whl`.
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "install_wheel.sh: usage: <wheel-runfile> <tag-runfile>" >&2
    exit 2
fi

# `$(rootpath ...)` arguments are relative to the launcher runfiles, not the
# workspace cwd used by the Windows rules_shell launcher.
# shellcheck source=runfiles_resolve.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/runfiles_resolve.sh"
WHEEL_RUNFILE="$(runfiles_resolve "$1")"
TAG_RUNFILE="$(runfiles_resolve "$2")"

if [[ ! -f "$WHEEL_RUNFILE" || ! -f "$TAG_RUNFILE" ]]; then
    echo "install_wheel.sh: expected runfiles missing" >&2
    echo "  wheel: $WHEEL_RUNFILE" >&2
    echo "  tag:   $TAG_RUNFILE" >&2
    exit 2
fi

tag="$(cat "$TAG_RUNFILE")"
if [[ -z "$tag" ]]; then
    echo "install_wheel.sh: wheel_tag.txt is empty — :_wheel_artifact failed to write the tag" >&2
    exit 2
fi

dst_dir="$BUILD_WORKSPACE_DIRECTORY/python/target/wheels"
mkdir -p "$dst_dir"
dst="$dst_dir/$tag"
cp -f "$WHEEL_RUNFILE" "$dst"
echo "wheel: $dst"
