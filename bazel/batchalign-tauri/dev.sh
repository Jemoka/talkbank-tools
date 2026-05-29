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

# PATH-prepend the Bazel-built cargo-tauri so the inner
# `cargo tauri dev` invocation never needs a host install.
CARGO_TAURI="$(rlocation _main/bazel/tauri/cargo-tauri)"
if [[ -z "$CARGO_TAURI" || ! -x "$CARGO_TAURI" ]]; then
    echo "dev.sh: rlocation could not resolve _main/bazel/tauri/cargo-tauri" >&2
    exit 2
fi
PATH="$(cd "$(dirname "$CARGO_TAURI")" && pwd):$PATH"
export PATH

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

# Stamp the build with the repo's BUILD_HASH so the Tauri shell can
# invalidate the PyApp install cache when this binary's build differs
# from the one that populated the cache (PyApp doesn't include feature
# flags in its own cache key). bazel/stamp.sh prints
#   BUILD_HASH <git-sha>[-dirty]
# We forward just the value via the BATCHALIGN_BUILD_HASH env var,
# which build.rs propagates into the binary as a rustc-env.
BATCHALIGN_BUILD_HASH="$(
    "$BUILD_WORKSPACE_DIRECTORY/bazel/stamp.sh" \
        | awk '/^BUILD_HASH/ {print $2; exit}'
)"
export BATCHALIGN_BUILD_HASH

exec cargo tauri dev "$@"
