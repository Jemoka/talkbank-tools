#!/usr/bin/env bash
# Pre-push hook: fast local checks that mirror CI gates.
# Install: ln -sf ../../scripts/pre-push.sh .git/hooks/pre-push
#
# Coverage goal: catch anything the GitHub "main CI" workflow would flag
# on a push to main, without running long test suites. If a CI job can
# fail purely because of committed content (not runtime behavior), this
# hook must cover it.
set -euo pipefail

echo "==> pre-push: fmt check"
cargo fmt --all -- --check
cd spec/tools && cargo fmt --all -- --check && cd ../..

echo "==> pre-push: affected compile check"
cargo run -q -p xtask -- affected-rust check

echo "==> pre-push: parser guardrail"
scripts/check-errorsink-option-signatures.sh

# Mirrors the "Generated Artifacts Up To Date" CI job.
echo "==> pre-push: generated artifacts up to date"
just spec gen-tree-sitter-tests >/dev/null
just spec gen-rust-tests >/dev/null
just spec gen-error-docs >/dev/null
generated_paths=(
    resources/spec/symbols/
    crates/talkbank-model/src/generated/
    crates/core/talkbank-spec-testgen/src/generated/
    crates/talkbank-parser-tests/tests/generated
    docs/errors
)
if ! git diff --quiet -- "${generated_paths[@]}"; then
    cat >&2 <<EOF
error: generated artifacts are out of sync with their specs.

The spec generators just regenerated these files in your working tree:
$(git diff --name-only -- "${generated_paths[@]}" | sed 's/^/  /')

You edited a spec but forgot to commit the regenerated output. Fix:

  git add ${generated_paths[*]}
  git commit --amend --no-edit   # or: git commit -m "regen"
  git push

EOF
    exit 1
fi

# Mirrors the "Fuzz Smoke Test" CI job's workspace discovery step.
echo "==> pre-push: fuzz workspace isolation"
(cd fuzz && cargo metadata --no-deps --format-version 1 >/dev/null)

if [[ "${TALKBANK_PRE_PUSH_CLIPPY:-0}" == "1" ]]; then
  echo "==> pre-push: affected clippy"
  cargo run -q -p xtask -- affected-rust clippy
fi

echo "✓ All pre-push checks passed"
