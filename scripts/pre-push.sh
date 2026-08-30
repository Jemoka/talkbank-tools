#!/usr/bin/env bash
# Pre-push hook: fast local checks that mirror CI gates.
# Install: ln -sf ../../scripts/pre-push.sh .git/hooks/pre-push
#
# Coverage goal: catch anything the GitHub "main CI" workflow would flag
# on a push to main, without running long test suites. If a CI job can
# fail purely because of committed content (not runtime behavior), this
# hook must cover it.
set -euo pipefail

echo "==> pre-push: format changed Rust files"
push_base="$(git merge-base HEAD '@{upstream}' 2>/dev/null || git rev-parse HEAD^)"
git diff --name-only --diff-filter=ACMR "$push_base"..HEAD -- '*.rs' |
  while IFS= read -r rust_file; do
    rustfmt --edition 2024 --check "$rust_file"
  done

echo "==> pre-push: Batchalign tests"
just batchalign test debug

echo "✓ All pre-push checks passed"
