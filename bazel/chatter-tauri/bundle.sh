#!/usr/bin/env bash
# Build the Chatter desktop app (Tauri v2) end-to-end via `cargo tauri build`.
#
# Profile selection follows Bazel's compilation_mode:
#   BAZEL_COMPILATION_MODE=opt (default) → cargo tauri build              (release bundle)
#   BAZEL_COMPILATION_MODE=dbg            → cargo tauri build --debug      (debug bundle)
# Override directly with TAURI_PROFILE={release|debug} if needed.
#
# Outputs:
#   release → apps/chatter/chatter-gui/src-tauri/target/release/bundle/
#   debug   → apps/chatter/chatter-gui/src-tauri/target/debug/bundle/
#
# cargo + cargo-tauri must be on $PATH (CI installs via setup actions).
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/chatter/chatter-gui"

case "${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE:-opt}}" in
    release|opt) profile_flag=() ;;
    debug|dbg|fastbuild) profile_flag=(--debug) ;;
    *) echo "unknown TAURI_PROFILE/BAZEL_COMPILATION_MODE: ${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE}}" >&2; exit 2 ;;
esac

exec cargo tauri build "${profile_flag[@]}" "$@"
