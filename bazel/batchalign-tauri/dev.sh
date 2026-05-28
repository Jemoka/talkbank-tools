#!/usr/bin/env bash
# `cargo tauri dev` with the sidecar daemon built + staged via Bazel.
#
# Mirrors bundle.sh's contract:
#   - SIDECAR_PATH (env, from $(rootpath //python/batchalign:sidecar))
#     is the sh_binary launcher for the sidecar target.
#   - We execute the launcher (escape-path: produces
#     python/target/pyapp/bin/sidecar), then stage that binary under
#     src-tauri/binaries/sidecar-<triple> so tauri.conf.json's
#     `bundle.externalBin = ["binaries/sidecar"]` resolves at compile
#     time.
#
# Difference from bundle.sh: ends with `cargo tauri dev` instead of
# `cargo tauri build`. Hot reload, no bundle output.

# --- begin runfiles.bash initialization v3 ---
# Bazel's canonical Bash runfiles library; see bundle.sh for the same.
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

# Resolve //python/batchalign:sidecar's launcher via Bazel's canonical
# runfiles helper. See bundle.sh for the same pattern + rationale.
SIDECAR_LAUNCHER="$(rlocation _main/python/batchalign/sidecar)"
if [[ -z "$SIDECAR_LAUNCHER" || ! -x "$SIDECAR_LAUNCHER" ]]; then
    echo "dev.sh: rlocation could not resolve _main/python/batchalign/sidecar" >&2
    exit 2
fi
echo "dev.sh: building sidecar via $SIDECAR_LAUNCHER"
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

# Frontend deps. cargo tauri dev expects `npm run dev` to start Vite
# (see tauri.conf.json's `beforeDevCommand`); npm needs node_modules.
if [[ ! -d node_modules ]]; then
    echo "dev.sh: node_modules missing → npm install"
    npm install
fi

exec cargo tauri dev "$@"
