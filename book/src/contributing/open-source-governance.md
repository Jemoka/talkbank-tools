# GitHub Readiness and Open Source Governance

**Status:** Current
**Last modified:** 2026-06-01 01:05 PDT

## Objective

Prepare `talkbank-tools` to operate as a healthy public project with
clear legal, security, contribution, and release processes.

## Root artifacts

| Artifact | Status | Notes |
|---|---|---|
| `LICENSE` | Done | BSD-3-Clause, `license.workspace = true` in all crates |
| `CONTRIBUTING.md` | Done | Setup, standards, PR flow, pre-PR checklist |
| `CODE_OF_CONDUCT.md` | Done | Contributor Covenant 2.1 with repo contact |
| `SECURITY.md` | Done | Issue-template contact link resolves to a real policy |
| `CODEOWNERS` | **TODO** | Not yet added; no path-level review ownership configured |
| `.github/workflows/` | Done (scoped) | Eight `bazel-*.yml` workflows + `publish-pypi.yml`; see [CI and Release](./ci-and-release.md) |
| `.github/ISSUE_TEMPLATE/*` | Done | Bug report + feature request (YAML forms) |
| Pull request template | Done | `.github/PULL_REQUEST_TEMPLATE.md` mirrors CONTRIBUTING + PR requirements |

## CI governance policy

The CI surface is a set of scoped `bazel-*.yml` workflows rather than a
single monolithic `ci.yml`. Each workflow watches a path filter and
runs the Bazel build / test targets relevant to that path. The full
`bazel build //... && bazel test //...` runs in
`bazel-build-all.yml` as a nightly cron (`0 6 * * *`) plus
on-dispatch.

- Required status checks: see
  [Branch Protection](./branch-protection.md) for the mapping of
  recommended checks to workflow jobs.
- There is no aggregate summary job (`ci-report`) today; required
  checks reference per-workflow jobs directly.
- The local pre-merge gate (`bazel build //... && bazel test //...`) and
  the pre-push hook (`scripts/pre-push.sh`) are documented in
  [Quality Gates](./quality-gates.md).

## Release governance

| Concern | Status |
|---|---|
| `batchalign3` PyPI publish workflow | Done (`publish-pypi.yml`, manual dispatch, OIDC trusted publisher) |
| Batchalign desktop bundle CI | Done (`bazel-tauri-batchalign.yml`); artifacts only; macOS + Linux; no GitHub Release upload; unsigned |
| Chatter CLI / `chatter-lsp` release workflow | **TODO** — built in CI by `bazel-rust.yml`, but no release workflow exists |
| Chatter desktop GUI workflow | **TODO** — only covered by nightly `bazel-build-all.yml` |
| VS Code extension release workflow | **TODO** — built and packaged by `bazel-typescript.yml`; marketplace publish is manual today (see [vscode/developer/releasing.md](../vscode/developer/releasing.md)) |
| Code signing / notarization | **TODO** on every surface; see [Code Signing and Distribution](../operations/code-signing-and-distribution.md) |
| Pre-1.0 release cadence and tagging | **TODO** |
| Changelog policy | **TODO** |

## Community operations

- Label taxonomy: `bug` and `enhancement` auto-applied by issue
  templates. Richer taxonomy (`drift`, `spec`, `grammar`, `parser`,
  `docs`, `good first issue`): **TODO** (GitHub settings).
- Contributor pathway: `CONTRIBUTING.md` covers setup and PR flow.
  First-time / advanced contributor pathways: **TODO**.
- Public project roadmap: **TODO**.

## Supply chain and security

- Dependency scanning: **TODO** — no `rustsec/audit-check` or
  `cargo-deny` job is wired into the current `bazel-*.yml` set.
  Automated update PRs (Dependabot / Renovate): **TODO**.
- Signed release artifacts: **TODO** on every platform.
- Security advisories process: documented in `SECURITY.md`.

## Acceptance criteria

- Repo has complete governance artifacts at root.
- CI workflows and branch protections enforce stated policy.
- Contributors can onboard and submit PRs without tribal knowledge.
- Release process is repeatable and documented (where it exists; the
  TODOs above are the gaps).

## See also

- [CI and Release](./ci-and-release.md)
- [Branch Protection](./branch-protection.md)
- [Quality Gates](./quality-gates.md)
- [Release Pipeline](../operations/release-pipeline.md)
- [Code Signing and Distribution](../operations/code-signing-and-distribution.md)
