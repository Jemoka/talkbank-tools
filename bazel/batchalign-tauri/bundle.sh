#!/usr/bin/env bash
# Build the Batchalign desktop app (Tauri v2) end-to-end via `cargo tauri build`.
#
# Pre-bundle step: stage the Bazel-built sidecar daemon (built by
# //python/batchalign:sidecar via PyApp) under
# src-tauri/binaries/sidecar-<target-triple>(.exe) so Tauri's
# `bundle.externalBin: ["binaries/sidecar"]` directive can resolve it.
# Tauri v2 appends the host target triple at bundle time; the staged
# filename MUST match exactly.
#
# Profile selection follows Bazel's compilation_mode:
#   BAZEL_COMPILATION_MODE=opt (default) → cargo tauri build              (release bundle)
#   BAZEL_COMPILATION_MODE=dbg            → cargo tauri build --debug      (debug bundle)
# Override directly with TAURI_PROFILE={release|debug} if needed.
#
# Outputs:
#   release → apps/batchalign/batchalign-gui/src-tauri/target/release/bundle/
#   debug   → apps/batchalign/batchalign-gui/src-tauri/target/debug/bundle/
#
# cargo + cargo-tauri must be on $PATH (CI installs via setup actions).
# SIDECAR_PATH is set by the Bazel sh_binary rule via env = {} so we
# don't have to hunt for it in runfiles ourselves.

set -euo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY/apps/batchalign/batchalign-gui"

# Detect target triple. `rustc -vV` emits `host: <triple>` on a line of
# its own; awk pulls just the triple. We need this both for staging the
# sidecar and for the Tauri bundler's --target flag (downstream).
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
if [[ -z "$TRIPLE" ]]; then
    echo "could not detect rustc host triple" >&2
    exit 2
fi

# Build the sidecar daemon binary by `bazel run`ing its target.
#
# We can't just execute //python/batchalign:sidecar's launcher directly:
# sh_binary's `args` (the $(rootpath ...) bindings pyapp_build.sh reads
# at $1..$6) are passed by `bazel run`, not baked into the launcher
# script. So we invoke bazel here, which:
#   - re-uses the same server (instant) if nothing changed in the dep
#     graph
#   - re-builds incrementally otherwise
#
# pyapp_build.sh is itself an escape-path build (host cargo + maturin
# can't live in Bazel's sandbox; see bazel/python/pyapp_build.sh). Its
# output lands at python/target/pyapp/bin/sidecar; we stage it from
# there below.
BAZEL="${BAZEL_REAL:-bazel}"
echo "bundle.sh: building sidecar via $BAZEL run //python/batchalign:sidecar"
"$BAZEL" run //python/batchalign:sidecar

SIDECAR_BINARY="$BUILD_WORKSPACE_DIRECTORY/python/target/pyapp/bin/sidecar"
if [[ ! -x "$SIDECAR_BINARY" ]]; then
    echo "sidecar launcher ran but did not produce $SIDECAR_BINARY" >&2
    exit 2
fi

mkdir -p src-tauri/binaries
TARGET="src-tauri/binaries/sidecar-${TRIPLE}"
case "$TRIPLE" in
    *windows*) TARGET="${TARGET}.exe" ;;
esac
cp -f "$SIDECAR_BINARY" "$TARGET"
chmod +x "$TARGET"

case "${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE:-opt}}" in
    release|opt) profile_flag=() ;;
    debug|dbg|fastbuild) profile_flag=(--debug) ;;
    *)
        echo "unknown TAURI_PROFILE/BAZEL_COMPILATION_MODE: ${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE}}" >&2
        exit 2
        ;;
esac

exec cargo tauri build "${profile_flag[@]}" "$@"
