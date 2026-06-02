# Testing and Quality Gates

**Status:** Current
**Last modified:** 2026-06-01 01:05 PDT

This page describes the **current** relationship between local
verification and CI. The local canonical gate is `bazel build //... &&
bazel test //...`. CI is a collection of scoped `bazel-*.yml` workflows
that mirror parts of that gate on the paths each workflow watches, plus
a nightly that runs the whole thing.

See [Testing](testing.md) for the canonical local gate definitions.

## Local pre-merge contract

`bazel build //... && bazel test //...` is the maintainer-facing local
contract. Run it before pushing any change that touches Rust, Python,
grammar, specs, or the book. The pre-push hook (`scripts/pre-push.sh`,
installed via `ln -sf ../../scripts/pre-push.sh .git/hooks/pre-push`)
runs the fast subset (fmt, affected compile, parser guardrail,
generated-check, fuzz-check) on every `git push`.

## CI workflow surface

| Workflow file | Jobs | When |
|---|---|---|
| `bazel-rust.yml` | `build-and-test` (ubuntu + macos-14) | push/PR on Rust paths |
| `bazel-python.yml` | `build-and-test` (ubuntu + macos-14) | push/PR on Python / engine paths |
| `bazel-grammar.yml` | `rust-binding`, `generated-artifacts-fresh` | push/PR on grammar / symbol paths |
| `bazel-typescript.yml` | `vscode-extension` (ubuntu) | push/PR on `apps/vscode-extension/**`, `schemas/**` |
| `bazel-docs.yml` | `build-and-linkcheck` (ubuntu) | push/PR on `book/**`, `bazel/book/**` |
| `bazel-tauri-batchalign.yml` | `smoke` (every PR), `bundle` (main + dispatch only, macOS arm64 / macOS x86_64 / Linux x86_64) | push/PR/dispatch on Batchalign GUI paths |
| `bazel-wheels.yml` | `build` (macos-14, macos-13, ubuntu, ubuntu-arm) | push/PR on wheel-affecting paths |
| `bazel-build-all.yml` | `build-all` (ubuntu + macos-14, full `//...`) | nightly cron `0 6 * * *` + manual dispatch |
| `publish-pypi.yml` | `verify-version`, `build`, `publish` | `workflow_dispatch` only |

CI is **not** a byte-for-byte mirror of the local gate sequence. Each
workflow runs the Bazel build + test targets relevant to its path
filter; the nightly `bazel-build-all.yml` covers everything that
per-path workflows skip.

## Gate-to-job mapping

| Concern | Local command | CI job(s) that cover it |
|---|---|---|
| Rust workspace compile + test | `bazel build //crates/... && bazel test //crates/...` | `bazel-rust.yml` `build-and-test`; nightly `bazel-build-all.yml` `build-all` |
| Grammar binding builds | `bazel build //grammar:tree_sitter_talkbank` | `bazel-grammar.yml` `rust-binding` |
| Generated `parser.c` / `grammar.json` / `node-types.json` fresh | `cd grammar && tree-sitter generate && git diff --exit-code` | `bazel-grammar.yml` `generated-artifacts-fresh` |
| Python wheel + engine cdylib + py_test | `bazel build //python/batchalign:wheel && bazel test //python/batchalign:pytest` | `bazel-python.yml` `build-and-test` |
| Wheel platform-matrix smoke (cp310-abi3) | `bazel run -c opt //python/batchalign:wheel` | `bazel-wheels.yml` `build` |
| VS Code extension build / test / package | `just vscode build`, `just vscode test`, `just vscode package` | `bazel-typescript.yml` `vscode-extension` |
| mdBook builds, internal links resolve | `bazel run //book:html && bazel run //book:linkcheck` | `bazel-docs.yml` `build-and-linkcheck` |
| Batchalign desktop bundle openapi snapshot fresh | `bazel test //apps/batchalign/batchalign-gui:test_openapi_freshness` | `bazel-tauri-batchalign.yml` `smoke` |
| Batchalign desktop bundle builds end-to-end | `bazel run //apps/batchalign/batchalign-gui/src-tauri:bundle -- --target <triple>` | `bazel-tauri-batchalign.yml` `bundle` (main + dispatch only) |
| Exhaustive `//...` build + test | `bazel build //... && bazel test //...` | `bazel-build-all.yml` `build-all` (nightly) |

## What's intentionally **not** enforced in CI

- **Python lockfile drift** (`//python:requirements_test`). The target
  exists but is tagged `manual` because `uv 0.5.18 --universal` is
  host-OS-dependent. Contributors regenerate the lockfile via
  `bazel run //python:requirements`; CI does not assert no drift.
  See the comment in `bazel-python.yml`.
- **Windows builds** (any surface). `tools/bazel` is a bash wrapper
  incompatible with PowerShell; `multitool.lock.json` has a `uv.exe`
  layout issue. Windows rows are commented out in
  `bazel-tauri-batchalign.yml` and `bazel-wheels.yml`. See
  [Code Signing and Distribution](../operations/code-signing-and-distribution.md).
- **Code signing / notarization.** Nothing is signed today.
- **Chatter desktop GUI bundle.** No dedicated workflow; only the
  nightly `bazel-build-all.yml` exercises it.

## Old job names that no longer exist

For anyone hunting through the git history: `rust-check-and-test`,
`spec-tools`, `chat-manual-anchor-check`, `generated-artifacts`,
`fuzz-smoke`, `vscode-vsix-smoke`, `cross-platform-smoke`,
`dependency-audit`, `semver-checks`, `ci-report` are all from the old
`ci.yml`. That workflow has been retired in favor of the scoped
`bazel-*.yml` set described above. The current job names live inside
those workflows (e.g. `build-and-test`, `rust-binding`,
`generated-artifacts-fresh`, `smoke`, `bundle`).
