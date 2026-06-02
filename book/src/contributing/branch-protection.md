# Branch Protection and Required CI Checks

**Status:** Current
**Last modified:** 2026-06-01 01:05 PDT

This page documents the **recommended** required-status-check set for
`main`. The current `.github/workflows/` set is a collection of scoped
`bazel-*.yml` workflows; required checks should reference real job
names from those files.

> Operators: the list below is what the docs prescribe. Whether each
> check is currently configured as required in GitHub branch protection
> is settings-side state and can only be confirmed by reading the
> repo's branch-protection rules directly.

## Branch protection policy

Enable branch protection for `main` with:

- Require pull request before merge.
- Require approvals (minimum 1; maintainers may set higher).
- Require conversation resolution before merge.
- Require status checks to pass before merge.
- Restrict force pushes and branch deletions.

## Required status checks

The following CI jobs should be required on `main`. Each entry names
the workflow file and the job within it; GitHub's branch-protection UI
exposes these as `"<workflow name> / <job name>"`.

| Workflow file | Workflow name (`name:` field) | Job | Why required |
|---|---|---|---|
| `bazel-rust.yml` | `bazel · rust` | `build-and-test` | Rust workspace compile + test |
| `bazel-python.yml` | `bazel · python` | `build-and-test` | Python wheel + engine + py_test |
| `bazel-grammar.yml` | `bazel · grammar` | `rust-binding` | Grammar Rust binding |
| `bazel-grammar.yml` | `bazel · grammar` | `generated-artifacts-fresh` | No drift in `grammar/src/parser.c` etc. |
| `bazel-typescript.yml` | `bazel · typescript` | `vscode-extension` | VS Code extension builds + tests |
| `bazel-docs.yml` | `bazel · docs` | `build-and-linkcheck` | mdBook + linkcheck pass |
| `bazel-tauri-batchalign.yml` | `bazel · tauri-batchalign` | `smoke` | Frontend + openapi snapshot drift |
| `bazel-wheels.yml` | `bazel · wheels (PR matrix)` | `build` | Multi-platform wheel build + abi3 smoke |

The `bundle` job in `bazel-tauri-batchalign.yml` only runs on `main`
and on dispatch (not on PRs), so it cannot be a PR-required check.
`bazel-build-all.yml` is a nightly cron and also cannot be a
PR-required check.

## Optional hardening

- Require branches to be up to date before merging.
- Enable merge queue if PR volume increases.
- Restrict who can dismiss stale reviews.

## Operational rule

If required checks fail:

- Do not bypass protection.
- Fix the issue (or revert the breaking change).
- Re-run checks until green.

## Old check names that no longer exist

References to `Rust Check and Test`, `Spec Tools Check and Test`,
`Grammar Generate and Test`, `Generated Artifacts Up To Date`,
`ci-report`, or the `ci.yml` workflow point at a retired CI layout.
The current workflows are the `bazel-*.yml` set described above.
