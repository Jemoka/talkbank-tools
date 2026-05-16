#!/usr/bin/env bash
# Publish all built wheels under python/target/wheels/ to PyPI via twine.
#
# Requires $PYPI_TOKEN (or ~/.pypirc) and a non-empty wheels directory.
# Use `just batchalign multiwheel` first to populate it.
#
# Hermeticity: this is a release-time operation; we still assert uv +
# python + maturin pins so the wheels we're about to upload were built
# with the pinned toolchain.
#
# $1 = uv binary (passed by sh_binary via @multitool//tools/uv).
set -euo pipefail
UV="$1"; shift

# shellcheck source=hermeticity_guard.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/hermeticity_guard.sh"
hermeticity_guard "$UV"

cd "$BUILD_WORKSPACE_DIRECTORY/python"

shopt -s nullglob
wheels=(target/wheels/*.whl)
if [[ ${#wheels[@]} -eq 0 ]]; then
    echo "twine_upload: no wheels found under target/wheels/" >&2
    echo "  fix: run 'just batchalign multiwheel' (or 'just batchalign wheel <triple>') first" >&2
    exit 1
fi

echo "twine_upload: uploading ${#wheels[@]} wheel(s):"
for w in "${wheels[@]}"; do printf '  %s\n' "$w"; done

# `--non-interactive` makes the absence of credentials fail fast rather
# than hanging on a tty prompt inside CI runners.
"$UV" run twine upload --non-interactive "$@" "${wheels[@]}"
