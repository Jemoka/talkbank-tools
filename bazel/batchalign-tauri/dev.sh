#!/usr/bin/env bash
# `cargo tauri dev` with the sidecar daemon supplied by Bazel.
#
# Mirrors bundle.sh's contract: resolve //python/batchalign:sidecar
# via rlocation, stage it under src-tauri/binaries/sidecar-<triple>,
# then hand off to cargo tauri dev. Hot reload, no bundler.

# --- begin runfiles.bash initialization v3 ---
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
    echo "dev.sh: rlocation could not resolve _main/python/batchalign/sidecar" >&2
    exit 2
fi

mkdir -p src-tauri/binaries
TARGET="src-tauri/binaries/sidecar-${TRIPLE}"
case "$TRIPLE" in
    *windows*) TARGET="${TARGET}.exe" ;;
esac
cp -f "$SIDECAR_BIN" "$TARGET"
chmod +x "$TARGET"

if [[ ! -d node_modules ]]; then
    echo "dev.sh: node_modules missing → npm install"
    npm install
fi

exec cargo tauri dev "$@"
