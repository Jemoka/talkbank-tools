#!/bin/bash
# Capture CLAN CHECK output for each check-error-corpus file.
#
# Produces tests/check-error-resources/corpus/golden/ with one .txt file per .cha file.
# Also produces tests/check-error-resources/corpus/golden/summary.tsv with error numbers.
#
# Usage:
#   bash scripts/capture_check_golden.sh [CLAN_CHECK_BINARY]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
CORPUS_DIR="$ROOT_DIR/tests/check-error-corpus"
GOLDEN_DIR="$CORPUS_DIR/golden"
CHECK_BIN="${1:-$ROOT_DIR/../OSX-CLAN/src/unix/bin/check}"

if [[ ! -x "$CHECK_BIN" ]]; then
    echo "ERROR: CLAN check binary not found at: $CHECK_BIN"
    echo "Usage: $0 [path/to/check]"
    exit 1
fi

mkdir -p "$GOLDEN_DIR"
SUMMARY="$GOLDEN_DIR/summary.tsv"
echo -e "file\texpected\tactual_errors\tstatus" > "$SUMMARY"

total=0
matched=0
mismatched=0
clean=0

for cha_file in "$CORPUS_DIR"/check_*.cha; do
    basename="$(basename "$cha_file" .cha)"
    golden_file="$GOLDEN_DIR/${basename}.txt"

    # Extract expected error number from filename (check_NNN)
    expected_num="${basename#check_}"
    expected_num="${expected_num#0}"
    expected_num="${expected_num#0}"

    # Run CLAN CHECK with file argument
    raw_output=$("$CHECK_BIN" "$cha_file" 2>&1 || true)

    # Extract error section: lines with *** File or error messages with (N)
    # CLAN outputs errors after the ******** separator
    error_lines=$(echo "$raw_output" | awk '
        /^\*\*\* File/ { print; next }
        /\([0-9]+\)$/ { print; next }
        # Also capture context lines between *** File and error message
        /^\*[A-Z]/ { print; next }
        /^%/ { print; next }
    ')

    # Also capture the full body between the two ******** blocks
    body=$(echo "$raw_output" | awk '
        BEGIN { count=0 }
        /^\*{8}/ { count++; next }
        count==1 { print }
    ')

    echo "$body" > "$golden_file"

    # Extract CHECK error numbers from output
    error_nums=$(echo "$body" | grep -oE '\([0-9]+\)' | tr -d '()' | sort -n | uniq | tr '\n' ',' | sed 's/,$//')

    total=$((total + 1))

    if echo "$body" | grep -q "ALL FILES CHECKED OUT OK"; then
        clean=$((clean + 1))
        echo -e "$basename\t$expected_num\t(clean)\tmiss" >> "$SUMMARY"
        printf "  MISS   %-20s expected (%s) but CLAN found no errors\n" "$basename" "$expected_num"
    elif echo "$error_nums" | grep -q "^${expected_num}\$\|,${expected_num},\|,${expected_num}\$\|^${expected_num},"; then
        matched=$((matched + 1))
        echo -e "$basename\t$expected_num\t$error_nums\tmatch" >> "$SUMMARY"
        printf "  MATCH  %-20s expected (%s), got [%s]\n" "$basename" "$expected_num" "$error_nums"
    else
        mismatched=$((mismatched + 1))
        echo -e "$basename\t$expected_num\t$error_nums\twrong" >> "$SUMMARY"
        printf "  WRONG  %-20s expected (%s), got [%s]\n" "$basename" "$expected_num" "$error_nums"
    fi
done

echo ""
echo "=== Summary ==="
echo "Total files:    $total"
echo "Matched:        $matched"
echo "Wrong error:    $mismatched"
echo "Clean (missed): $clean"
echo ""
echo "Golden output:  $GOLDEN_DIR/"
echo "Summary TSV:    $SUMMARY"
