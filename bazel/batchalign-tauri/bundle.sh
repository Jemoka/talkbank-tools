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

# --- begin runfiles.bash initialization v3 ---
# Bazel's canonical Bash runfiles library. Provides `rlocation <path>`
# which resolves any runfiles input (declared via `data` on this
# sh_binary) regardless of whether Bazel set RUNFILES_DIR, a manifest
# file, or neither. Copy-pasted verbatim from the upstream boilerplate;
# see https://github.com/bazelbuild/bazel/blob/master/tools/bash/runfiles/runfiles.bash
set -uo pipefail; set +e; f=bazel_tools/tools/bash/runfiles/runfiles.bash
# shellcheck disable=SC1090
source "${RUNFILES_DIR:-/dev/null}/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "${RUNFILES_MANIFEST_FILE:-/dev/null}" | cut -f2- -d' ')" 2>/dev/null || \
  source "$0.runfiles/$f" 2>/dev/null || \
  source "$(grep -sm1 "^$f " "$0.runfiles_manifest" | cut -f2- -d' ')" 2>/dev/null || \
  { echo>&2 "ERROR: cannot find $f"; exit 1; }; f=; set -e
# --- end runfiles.bash initialization v3 ---
set -o pipefail

cd "$BUILD_WORKSPACE_DIRECTORY/apps/batchalign/batchalign-gui"

# Detect target triple. `rustc -vV` emits `host: <triple>` on a line of
# its own; awk pulls just the triple. We need this both for staging the
# sidecar and for the Tauri bundler's --target flag (downstream).
TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
if [[ -z "$TRIPLE" ]]; then
    echo "could not detect rustc host triple" >&2
    exit 2
fi

# Build the sidecar daemon binary via its sh_binary launcher.
#
# //python/batchalign:sidecar is a `sh_binary` (escape-path build —
# pyapp_build.sh needs the host cargo + a writable target dir, which
# can't live in the Bazel sandbox; see bazel/python/pyapp_build.sh).
# Bazel materializes the launcher script into our runfiles tree because
# we list it as `data` on this sh_binary. Resolve its path via the
# canonical `rlocation` helper from runfiles.bash and execute it —
# pyapp_build.sh produces python/target/pyapp/bin/sidecar (escape
# path), which we then stage.
SIDECAR_LAUNCHER="$(rlocation _main/python/batchalign/sidecar)"
if [[ -z "$SIDECAR_LAUNCHER" || ! -x "$SIDECAR_LAUNCHER" ]]; then
    echo "bundle.sh: rlocation could not resolve _main/python/batchalign/sidecar" >&2
    exit 2
fi
echo "bundle.sh: building sidecar via $SIDECAR_LAUNCHER"
"$SIDECAR_LAUNCHER"

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
