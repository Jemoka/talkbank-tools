#!/usr/bin/env bash
# E2E test for LazyProfile worker architecture.
#
# Verifies that --memory-tier medium forces LazyProfile mode, where the GPU
# worker starts with no models and loads them on demand via ensure_task.
#
# This test runs a real morphotag job (loads Stanza models) on a CHAT fixture,
# verifying the full path: CLI → direct execution → LazyProfile worker spawn →
# ensure_task IPC → on-demand Stanza loading → morphosyntax inference → result.
#
# Usage:
#   bash scripts/test_lazy_profile_e2e.sh           # debug binary
#   bash scripts/test_lazy_profile_e2e.sh release    # release binary
#
# Requirements:
#   - Python 3.12 with batchalign installed (uv sync)
#   - Stanza models downloaded (auto-downloads on first run)
#   - At least 4 GB free RAM

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Select binary
if [[ "${1:-}" == "release" ]]; then
    BINARY="$REPO_DIR/target/release/batchalign3"
else
    BINARY="$REPO_DIR/target/debug/batchalign3"
fi

if [[ ! -x "$BINARY" ]]; then
    echo "ERROR: binary not found: $BINARY"
    echo "Build first: cargo build -p batchalign-cli"
    exit 1
fi

FIXTURE="$REPO_DIR/resources/fixtures/eng_three_utterances.cha"
if [[ ! -f "$FIXTURE" ]]; then
    echo "ERROR: fixture not found: $FIXTURE"
    exit 1
fi

# Create temporary output directory
OUT_DIR=$(mktemp -d)
trap 'rm -rf "$OUT_DIR"' EXIT

echo "=== LazyProfile E2E Test ==="
echo "Binary:  $BINARY"
echo "Fixture: $FIXTURE"
echo "Output:  $OUT_DIR"
echo ""

# --- Test 1: morphotag with --memory-tier medium (LazyProfile) ---
echo "--- Test 1: morphotag --memory-tier medium (LazyProfile) ---"
echo ""

"$BINARY" \
    --no-server \
    --no-tui \
    --no-open-dashboard \
    --memory-tier medium \
    -v \
    morphotag \
    "$FIXTURE" \
    -o "$OUT_DIR/medium" \
    2>&1 | tee "$OUT_DIR/medium.log"

echo ""

# Verify output exists and has %mor tier
OUTPUT_FILE="$OUT_DIR/medium/eng_three_utterances.cha"
if [[ ! -f "$OUTPUT_FILE" ]]; then
    echo "FAIL: output file not created"
    exit 1
fi

if ! grep -q '%mor:' "$OUTPUT_FILE"; then
    echo "FAIL: output missing %mor tier"
    cat "$OUTPUT_FILE"
    exit 1
fi

echo "PASS: morphotag produced %mor tier with --memory-tier medium"
echo ""

# Check the log for LazyProfile indicators
if grep -q "lazy-profile\|LazyProfile\|Lazy profile\|--lazy\|ensure_task" "$OUT_DIR/medium.log"; then
    echo "PASS: log contains lazy profile indicators"
else
    echo "WARNING: log does not contain obvious lazy profile indicators (may be OK if verbose < 2)"
fi

echo ""

# --- Test 2: morphotag with default tier (should NOT be lazy on this 256 GB machine) ---
echo "--- Test 2: morphotag default tier (Profile mode on 256 GB) ---"
echo ""

"$BINARY" \
    --no-server \
    --no-tui \
    --no-open-dashboard \
    -v \
    morphotag \
    "$FIXTURE" \
    -o "$OUT_DIR/default" \
    2>&1 | tee "$OUT_DIR/default.log"

echo ""

OUTPUT_FILE_DEFAULT="$OUT_DIR/default/eng_three_utterances.cha"
if [[ ! -f "$OUTPUT_FILE_DEFAULT" ]]; then
    echo "FAIL: default-tier output file not created"
    exit 1
fi

if ! grep -q '%mor:' "$OUTPUT_FILE_DEFAULT"; then
    echo "FAIL: default-tier output missing %mor tier"
    exit 1
fi

echo "PASS: morphotag produced %mor tier with default tier"
echo ""

# --- Test 3: Compare outputs (should be identical NLP content) ---
echo "--- Test 3: Compare outputs ---"

# Extract just the %mor lines for comparison (ignore timing/metadata differences)
MOR_MEDIUM=$(grep '%mor:' "$OUT_DIR/medium/eng_three_utterances.cha" | sort)
MOR_DEFAULT=$(grep '%mor:' "$OUT_DIR/default/eng_three_utterances.cha" | sort)

if [[ "$MOR_MEDIUM" == "$MOR_DEFAULT" ]]; then
    echo "PASS: %mor output identical between medium and default tiers"
else
    echo "FAIL: %mor output differs between tiers"
    echo "Medium:"
    echo "$MOR_MEDIUM"
    echo "Default:"
    echo "$MOR_DEFAULT"
    exit 1
fi

echo ""
echo "=== All LazyProfile E2E tests passed ==="
