#!/bin/bash
# Test a single CHAT file for roundtrip
#
# Usage:
#   ./scripts/test-single-file.sh action.cha
#   ./scripts/test-single-file.sh /full/path/to/file.cha

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <filename.cha> [corpus_dir]"
    echo ""
    echo "Examples:"
    echo "  $0 action.cha                    # Test from default corpus"
    echo "  $0 /path/to/file.cha             # Test specific file"
    echo "  $0 file.cha ~/my-corpus          # Test with custom corpus dir"
    exit 1
fi

FILE="$1"
CORPUS_ARG="$2"

# If it's a full path, extract just the filename and use its directory as corpus_dir
if [[ "$FILE" == /* ]]; then
    TEST_FILE=$(basename "$FILE")
    CORPUS_DIR=$(dirname "$FILE")
else
    TEST_FILE="$FILE"
    REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
    CORPUS_DIR="${CORPUS_ARG:-$REPO_ROOT/resources/corpus/reference}"
fi

echo "Testing: $TEST_FILE"
echo "Corpus:  $CORPUS_DIR"
echo ""

# Run the test using clap arguments
echo "Running test..."
cargo test --test single_file_roundtrip -- --file "$TEST_FILE" --corpus-dir "$CORPUS_DIR"