#!/usr/bin/env bash
# Build the Batchalign desktop app (Tauri v2) end-to-end via `cargo tauri build`.
#
# Bazel hands us the sidecar daemon binary as a real file output —
# //python/batchalign:sidecar is a genrule that vendors pyapp via
# git_repository, embeds the Bazel-tracked wheel, and produces a
# tracked binary. We resolve its path via rlocation, stage it under
# src-tauri/binaries/sidecar-<triple> for Tauri's externalBin contract,
# then shell out to `cargo tauri build`.

# --- begin runfiles.bash initialization v3 ---
# Bazel's canonical Bash runfiles library; provides `rlocation`. See
# https://github.com/bazelbuild/bazel/blob/master/tools/bash/runfiles/runfiles.bash
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

TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
if [[ -z "$TRIPLE" ]]; then
    echo "could not detect rustc host triple" >&2
    exit 2
fi

SIDECAR_BIN="$(rlocation _main/python/batchalign/sidecar)"
if [[ -z "$SIDECAR_BIN" || ! -x "$SIDECAR_BIN" ]]; then
    echo "bundle.sh: rlocation could not resolve _main/python/batchalign/sidecar" >&2
    exit 2
fi

mkdir -p src-tauri/binaries
TARGET="src-tauri/binaries/sidecar-${TRIPLE}"
case "$TRIPLE" in
    *windows*) TARGET="${TARGET}.exe" ;;
esac
cp -f "$SIDECAR_BIN" "$TARGET"
chmod +x "$TARGET"

# Profile selection. Bazel propagates COMPILATION_MODE; TAURI_PROFILE
# overrides if set directly (the justfile passes it).
case "${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE:-opt}}" in
    release|opt)        profile_flag=() ;;
    debug|dbg|fastbuild) profile_flag=(--debug) ;;
    *)
        echo "unknown TAURI_PROFILE/BAZEL_COMPILATION_MODE: ${TAURI_PROFILE:-${BAZEL_COMPILATION_MODE}}" >&2
        exit 2
        ;;
esac

exec cargo tauri build "${profile_flag[@]}" "$@"
