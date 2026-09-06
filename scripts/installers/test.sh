#!/bin/bash
# scripts/installers/test.sh — Local integration test for one-click installer scripts.
#
# Builds a wheel from the local checkout, installs it via the macOS installer
# script into an isolated uv tool directory, verifies the CLI works, then
# tests the upgrade (re-install) path. Cleans up on exit.
#
# Prerequisites:
#   - uv, Rust toolchain
#   - run from the Batchalign repository root
#
# Usage:
#   bash scripts/installers/test.sh              # run from repo root
#   bash scripts/installers/test.sh --no-build   # skip wheel build, reuse previous

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$REPO_ROOT/dist"
BUILD_WHEEL=true

for arg in "$@"; do
    case "$arg" in
        --no-build) BUILD_WHEEL=false ;;
        *) echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

# ── Build wheel ─────────────────────────────────────────────────────────────

if [ "$BUILD_WHEEL" = true ]; then
    echo "=== Building wheel ==="
    cd "$REPO_ROOT"
    uv build --wheel --out-dir "$DIST_DIR/"
    echo ""
fi

WHEEL="$(ls -t "$DIST_DIR"/*.whl 2>/dev/null | head -1)"
if [ -z "$WHEEL" ]; then
    echo "ERROR: No wheel found in $DIST_DIR/. Run without --no-build."
    exit 1
fi
echo "Using wheel: $WHEEL"
echo ""

# ── Set up isolated environment ─────────────────────────────────────────────

SANDBOX="$(mktemp -d)"
export UV_TOOL_DIR="$SANDBOX/tools"
export UV_TOOL_BIN_DIR="$SANDBOX/bin"
export PATH="$UV_TOOL_BIN_DIR:$PATH"

cleanup() {
    echo ""
    echo "=== Cleaning up ==="
    rm -rf "$SANDBOX"
    echo "[OK] Removed $SANDBOX"
}
trap cleanup EXIT

echo "Sandbox: $SANDBOX"
echo "UV_TOOL_DIR=$UV_TOOL_DIR"
echo "UV_TOOL_BIN_DIR=$UV_TOOL_BIN_DIR"
echo ""

# ── Test 1: Fresh install ───────────────────────────────────────────────────

echo "=== Test 1: Fresh install via installer script ==="
BATCHALIGN_PACKAGE="$WHEEL" CI=true bash "$SCRIPT_DIR/macos/install-batchalign3.command"
echo ""

# ── Test 2: Verify CLI works ────────────────────────────────────────────────

echo "=== Test 2: Verify batchalign3 --help ==="
batchalign3 --help >/dev/null
echo "[OK] batchalign3 --help exits 0"

echo ""
echo "=== Test 3: Verify batchalign3 version ==="
batchalign3 version 2>&1 || true
echo ""

# ── Test 4: Upgrade (re-run installer) ──────────────────────────────────────

echo "=== Test 4: Upgrade via installer script (re-run) ==="
BATCHALIGN_PACKAGE="$WHEEL" CI=true bash "$SCRIPT_DIR/macos/install-batchalign3.command"
echo ""

# ── Test 5: Verify CLI still works after upgrade ────────────────────────────

echo "=== Test 5: Verify batchalign3 --help after upgrade ==="
batchalign3 --help >/dev/null
echo "[OK] batchalign3 --help exits 0 after upgrade"
echo ""

# ── Done ────────────────────────────────────────────────────────────────────

echo "============================================"
echo "  All installer tests passed!"
echo "============================================"
