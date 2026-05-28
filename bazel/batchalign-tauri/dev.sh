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

set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY/apps/batchalign/batchalign-gui"

TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
if [[ -z "$TRIPLE" ]]; then
    echo "could not detect rustc host triple" >&2
    exit 2
fi

SIDECAR_LAUNCHER="${SIDECAR_PATH:-}"
if [[ -z "$SIDECAR_LAUNCHER" ]]; then
    echo "SIDECAR_PATH env var not set (Bazel env= attribute missing?)" >&2
    exit 2
fi
if [[ ! -x "$SIDECAR_LAUNCHER" ]]; then
    if [[ -x "$BUILD_WORKSPACE_DIRECTORY/$SIDECAR_LAUNCHER" ]]; then
        SIDECAR_LAUNCHER="$BUILD_WORKSPACE_DIRECTORY/$SIDECAR_LAUNCHER"
    else
        echo "sidecar launcher not executable at $SIDECAR_LAUNCHER" >&2
        exit 2
    fi
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
