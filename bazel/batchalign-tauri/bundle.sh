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

# Resolve SIDECAR_PATH against the workspace dir so the Bazel rootpath
# expansion (relative path) works.
SIDECAR_SRC="${SIDECAR_PATH:-}"
if [[ -z "$SIDECAR_SRC" ]]; then
    echo "SIDECAR_PATH env var not set (Bazel env= attribute missing?)" >&2
    exit 2
fi
if [[ ! -f "$SIDECAR_SRC" ]]; then
    # rootpath gave us workspace-relative; try resolving from cwd.
    if [[ -f "$BUILD_WORKSPACE_DIRECTORY/$SIDECAR_SRC" ]]; then
        SIDECAR_SRC="$BUILD_WORKSPACE_DIRECTORY/$SIDECAR_SRC"
    else
        echo "sidecar binary not found at $SIDECAR_SRC" >&2
        exit 2
    fi
fi

mkdir -p src-tauri/binaries
TARGET="src-tauri/binaries/sidecar-${TRIPLE}"
case "$TRIPLE" in
    *windows*) TARGET="${TARGET}.exe" ;;
esac
cp -f "$SIDECAR_SRC" "$TARGET"
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
