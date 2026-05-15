# Testing and Quality Gates

**Status:** Current
**Last updated:** 2026-04-29 10:39 EDT

This page summarizes the **current** relationship between the local pre-merge
gate (`bazel build //... && bazel test //...`) and the root CI workflow (`.github/workflows/ci.yml`).
See [Testing](testing.md) for the canonical local gate definitions.

## Local pre-merge contract

`bazel build //... && bazel test //...` is the maintainer-facing local contract. It runs gates G0–G14 in
sequence. `hooks-check` runs first as a warning, but it is not a numbered gate.

## Root CI contract

Root CI is broader than `bazel build //... && bazel test //...`, but it is **not** a byte-for-byte mirror
of the local gate sequence. The workflow includes local-contract coverage where
practical, plus CI-only jobs such as grammar generation, reference-corpus
roundtrip, VS Code jobs, cross-platform CLI smoke, dependency audit, and the
aggregate `ci-report`.

### Local gate coverage in CI

| Local gate | Local command | CI coverage today |
|---|---|---|
| G0 | `bash scripts/check-errorsink-option-signatures.sh` | `rust-check-and-test` |
| G1 | `cargo check --workspace --all-targets` | `rust-check-and-test` |
| G2 | `cd spec/tools && cargo check --all-targets` | `spec-tools` |
| G3 | `cargo check --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml --all-targets` | **Not mirrored in root CI** |
| G4 | `bash scripts/check-chat-manual-anchors.sh` | `chat-manual-anchor-check` |
| G5 | `cargo nextest run -p talkbank-parser-tests --test generated` | `rust-check-and-test` |
| G6 | `bazel test //crates/core/talkbank-parser-tests/...` | `rust-check-and-test` |
| G7 | `cargo nextest run --test bare_timestamp_regression` | `rust-check-and-test` |
| G8 | `cargo nextest run -p talkbank-parser-tests --test parser_equivalence_files` | `rust-check-and-test` |
| G9 | `cargo nextest run -p talkbank-parser-tests --test wor_terminator_alignment` | `rust-check-and-test` |
| G10 | `cargo nextest run -p talkbank-parser-tests --test parser_suite` | `rust-check-and-test` |
| G11 | `just spec coverage` | **Not mirrored in root CI** |
| G12 | `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code` | `generated-artifacts` |
| G13 | `(cd fuzz && cargo metadata --no-deps --format-version 1 >/dev/null)` | Covered more broadly by `fuzz-smoke`, not by the same command |
| G14 | `bazel build //crates/batchalign/... && bazel test //crates/batchalign/...` | **Not mirrored in root CI** |

### Additional CI-only checks

These are required CI signals but are not part of `bazel build //... && bazel test //...`:

- `grammar`
- `reference-corpus-roundtrip`
- `vscode` and `vscode-vsix-smoke`
- `cross-platform-smoke`
- `dependency-audit`
- `semver-checks` (pull requests)
- `ci-report`
