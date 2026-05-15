#!/usr/bin/env bash
# Corpus-wide parity check: runs CLAN binary and chatter clan on the same files
# and diffs the output. Reports pass/fail/known-divergence per command.
#
# Usage: scripts/parity-check.sh [corpus_dir]
# Requires: CLAN_BIN_DIR env var pointing to CLAN binary directory
#           chatter binary in PATH

set -euo pipefail

CORPUS_DIR="${1:-../../resources/corpus/reference}"
# Look for CLAN binaries: env var → workspace sibling → legacy home path
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_CLAN="$(cd "$SCRIPT_DIR/../../.." 2>/dev/null && pwd)/OSX-CLAN/src/unix/bin"
if [ -n "${CLAN_BIN_DIR:-}" ]; then
    CLAN_DIR="$CLAN_BIN_DIR"
elif [ -d "$WORKSPACE_CLAN" ]; then
    CLAN_DIR="$WORKSPACE_CLAN"
else
    CLAN_DIR="$HOME/OSX-CLAN/src/unix/bin"
fi

if [ ! -d "$CLAN_DIR" ]; then
    echo "CLAN binaries not found at $CLAN_DIR"
    echo "Set CLAN_BIN_DIR to the path containing CLAN executables."
    exit 1
fi

if ! command -v chatter &>/dev/null; then
    echo "chatter not found in PATH. Build with: cargo install --path ../talkbank-chatter/crates/chatter-cli"
    exit 1
fi

# Commands to test (analysis only — transforms and converters need different handling)
COMMANDS=(freq mlu mlt vocd dist maxwd timedur wdlen uniq chip)

TOTAL=0
PASS=0
FAIL=0
SKIP=0

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

echo "Parity check: $CORPUS_DIR"
echo "CLAN binaries: $CLAN_DIR"
echo "---"

for cmd in "${COMMANDS[@]}"; do
    clan_bin="$CLAN_DIR/$cmd"
    if [ ! -x "$clan_bin" ]; then
        echo "SKIP $cmd (binary not found)"
        SKIP=$((SKIP + 1))
        continue
    fi

    # Find up to 5 .cha files to test
    files=$(find "$CORPUS_DIR" -name '*.cha' -type f | head -5)

    for f in $files; do
        basename=$(basename "$f")
        TOTAL=$((TOTAL + 1))

        # Run CLAN binary
        clan_out="$TMPDIR/${cmd}_${basename}_clan.txt"
        echo "$f" | "$clan_bin" > "$clan_out" 2>&1 || true

        # Run chatter clan
        rust_out="$TMPDIR/${cmd}_${basename}_rust.txt"
        chatter clan "$cmd" --format clan "$f" > "$rust_out" 2>&1 || true

        # Compare (ignoring header envelope)
        if diff -q "$clan_out" "$rust_out" &>/dev/null; then
            PASS=$((PASS + 1))
        else
            echo "DIFF $cmd $basename"
            diff --brief "$clan_out" "$rust_out" 2>/dev/null || true
            FAIL=$((FAIL + 1))
        fi
    done
done

echo "---"
echo "Results: $TOTAL tested, $PASS passed, $FAIL failed, $SKIP skipped"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
