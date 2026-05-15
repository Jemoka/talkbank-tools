#!/usr/bin/env bash
# Verify every error code defined in error_code.rs has a corresponding spec file.
#
# Error codes are defined with #[code("E###")] attributes in:
#   crates/talkbank-model/src/errors/codes/error_code.rs
#
# Spec files live in resources/spec/errors/ as E###_*.md

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ERROR_CODE_FILE="$REPO_ROOT/crates/talkbank-model/src/errors/codes/error_code.rs"
SPEC_DIR="$REPO_ROOT/spec/errors"

if [ ! -f "$ERROR_CODE_FILE" ]; then
    echo "ERROR: Cannot find $ERROR_CODE_FILE"
    exit 1
fi

# Extract all error codes from #[code("E###")] attributes
# Use sed instead of grep -P for macOS compatibility
CODES=$(grep '#\[code("' "$ERROR_CODE_FILE" | sed 's/.*#\[code("\([EW][0-9]*\)").*/\1/' | sort -u)

MISSING=0
for code in $CODES; do
    # Check for spec file matching E###*.md or W###*.md (with or without _ suffix)
    if ! ls "$SPEC_DIR/${code}"*.md >/dev/null 2>&1; then
        echo "MISSING: $code — no spec file in resources/spec/errors/"
        MISSING=$((MISSING + 1))
    fi
done

TOTAL=$(echo "$CODES" | wc -w)
FOUND=$((TOTAL - MISSING))

echo ""
echo "Error codes: $TOTAL total, $FOUND have specs, $MISSING missing"

if [ "$MISSING" -gt 0 ]; then
    echo ""
    echo "Create missing specs with: resources/spec/errors/<CODE>_<description>.md"
    echo "Then run: just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs"
    exit 1
fi
