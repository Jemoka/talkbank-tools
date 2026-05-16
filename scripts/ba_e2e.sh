#!/usr/bin/env bash
# scripts/ba_e2e.sh — end-to-end smoke run of the four spec2.md scenarios.
#
# Usage:
#   bash scripts/ba_e2e.sh [INPUT_DIR] [OUTPUT_DIR]
#
# Default INPUT_DIR is /Users/houjun/Documents/Projects/talkbank-alignment/input
# Default OUTPUT_DIR is /tmp/ba_e2e_out
#
# Scenarios run:
#   1. compare       TD020.cha vs template.gold.cha → main.cha with %xref/%xcmp
#   2. morphotag     TD020.cha with retokenize=False
#   3. morphotag     TD020.cha with retokenize=True
#   4. utseg         TD020.cha (re-segment the existing utterances)
#   5. transcribe    calderone.mp3 (ASR; uses WhisperBackend if available)
#
# The script gracefully skips scenarios whose backends require unavailable
# models (Stanza English, Whisper) and prints a clear status.

set -uo pipefail

IN="${1:-/Users/houjun/Documents/Projects/talkbank-alignment/input}"
OUT="${2:-/tmp/ba_e2e_out}"
mkdir -p "$OUT"

run() {
    local name="$1"; shift
    echo
    echo "=== $name ==="
    if "$@"; then
        echo "PASS: $name"
    else
        echo "FAIL: $name (exit $?)"
    fi
}

run "compare"                batchalign3 compare \
    "$IN/TD020.cha" --gold "$IN/template.gold.cha" --out "$OUT"

run "morphotag (no retokenize)"  batchalign3 morphotag \
    "$IN/TD020.cha" --out "$OUT/morphotag_no_retokenize" --no-retokenize

run "morphotag (retokenize)"     batchalign3 morphotag \
    "$IN/TD020.cha" --out "$OUT/morphotag_retokenize" --retokenize

run "utseg"                  batchalign3 utseg \
    "$IN/TD020.cha" --out "$OUT/utseg"

run "transcribe"             batchalign3 transcribe \
    "$IN/calderone.mp3" --out "$OUT/transcribe" --no-fa

echo
echo "All scenarios attempted; output in $OUT"
