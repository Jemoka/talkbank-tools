#!/usr/bin/env bash
# Build the batchalign wheel via maturin.
#
# Profile selection (release | dev) follows Bazel's compilation_mode:
#   BAZEL_COMPILATION_MODE=opt (default) → maturin --release
#   BAZEL_COMPILATION_MODE=dbg            → maturin (no flag, dev profile)
# Override directly with MATURIN_PROFILE={release|dev} if needed.
#
# Platform targeting: set MATURIN_TARGET to a Rust target triple to
# cross-compile (e.g. `MATURIN_TARGET=aarch64-apple-darwin`). Defaults to
# the host triple. Used by the justfile's per-platform wheel recipes.
#
# Hermeticity: the guard script asserts uv/maturin/python/rustc versions
# match the pins in pyproject.toml before any host-tool invocation.
#
# $1 = uv binary (passed by sh_binary via @multitool//tools/uv).
set -euo pipefail
UV="$1"; shift

# shellcheck source=hermeticity_guard.sh
source "${BUILD_WORKSPACE_DIRECTORY}/bazel/python/hermeticity_guard.sh"
hermeticity_guard "$UV"
UV="$HERMETIC_UV"

cd "$BUILD_WORKSPACE_DIRECTORY/python"

case "${MATURIN_PROFILE:-${BAZEL_COMPILATION_MODE:-opt}}" in
    release|opt) profile_flag=(--release) ;;
    dev|dbg|fastbuild) profile_flag=() ;;
    *) echo "unknown MATURIN_PROFILE/BAZEL_COMPILATION_MODE: ${MATURIN_PROFILE:-${BAZEL_COMPILATION_MODE}}" >&2; exit 2 ;;
esac

target_flag=()
if [[ -n "${MATURIN_TARGET:-}" ]]; then
    target_flag=(--target "$MATURIN_TARGET")
fi

# `"${arr[@]+"${arr[@]}"}"` is the bash-set-u-safe way to splat a
# possibly-empty array — naked `"${arr[@]}"` trips `unbound variable`
# in bash 4.4+ when the array has zero elements.
"$UV" run maturin build \
    "${profile_flag[@]+"${profile_flag[@]}"}" \
    "${target_flag[@]+"${target_flag[@]}"}" \
    --manifest-path ../crates/batchalign/batchalign-engine/Cargo.toml \
    --out target/wheels \
    "$@"
ls -lh target/wheels/ || true
