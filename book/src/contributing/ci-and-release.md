# CI and Release

**Status:** Current
**Last updated:** 2026-04-29 10:39 EDT

## Pre-Merge Verification

Every change must pass `bazel build //... && bazel test //...` before merging. This runs gates G0 through G14:

```bash
bazel build //... && bazel test //...
```

See [Testing > Verification Gates](testing.md#verification-gates) for the full gate table.

## Generated Artifact Check (G12)

`just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code` regenerates all artifacts and verifies they match what's committed:

- Symbol sets (Rust and JavaScript)
- Tree-sitter corpus tests
- Rust parser tests
- Error documentation

If the check fails, it means committed generated artifacts are out of sync with
their source inputs. That usually means specs or symbols changed without the
corresponding regeneration step, not that every parser change should have run
`just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`.

## Parser Signature Guardrail (G0)

`bash scripts/check-errorsink-option-signatures.sh` enforces a coding convention: parser functions should use consistent `ErrorSink` signatures. This prevents accidental introduction of incompatible parser APIs.

## Release Process

Releases are currently manual and split by surface. Use the workflow that
matches the release contract for the surface you are shipping.

| Workflow | Scope | Tier | Output | Guardrails |
|----------|-------|------|--------|------------|
| `.github/workflows/release.yml` (`TalkBank Core Release`) | Canonical Rust binary release line for `chatter`; bundles preview `chatter-lsp` alongside it | Public stable + preview convenience binary | GitHub Release assets for the shared Rust workspace version | Requires an existing `vX.Y.Z` tag matching `Cargo.toml`; runs native binary smoke tests before publishing; does **not** cargo-publish the Rust library crates |
| `.github/workflows/batchalign-release.yml` (`Batchalign Package Release (Preview)`) | Public `batchalign3` package/CLI release line | Public preview | Wheels + sdist, with optional GitHub Release and optional PyPI publish | Requires an existing `vX.Y.Z` tag matching `pyproject.toml`; runs wheel smoke tests before publish |
| `.github/workflows/vscode-release.yml` (`VS Code Extension Release (Preview)`) | VS Code extension release line only | Public preview | GitHub prerelease containing five platform-specific VSIX files; no Marketplace publish | Requires `apps/vscode-extension/package.json` version match; compiles, lints, and tests before packaging |
| `.github/workflows/batchalign-desktop.yml` (`Batchalign Desktop Bundles (Experimental)`) | Dashboard desktop shell bundle builds | Experimental | GitHub Actions artifacts only | Artifact-only internal validation; does not create a public GitHub release |

General process:

1. Run the relevant verification for the surface you are releasing (`bazel build //... && bazel test //...`
   for the core Rust release line, the built-in workflow checks for VS Code, and
   the Batchalign packaging checks for `batchalign3`).
2. Update the canonical version source for that surface.
3. For the core Rust and Batchalign package release lines, create and push the
   matching `vX.Y.Z` git tag before dispatching the workflow.
4. Trigger the surface-specific workflow.
5. Only promote a preview or experimental surface by updating
   `book/src/operations/release-contract.md`, `book/src/operations/versioning.md`, and the corresponding
   workflow labeling together.

## First-release release-ops guardrails

- **VS Code stays GitHub Releases VSIX-only for the first public release.** Do
  not add Marketplace publishing steps or docs until the release contract and
  workflow/docs are explicitly reopened together.
- **Batchalign release smoke must cover `batchalign3 serve` + `/health`.** The
  wheel smoke path is not complete if it only checks `--help` and `version`.
- **Signing/notarization language must follow `book/src/operations/code-signing-and-distribution.md`.**
  Current GitHub Release archives, wheels, and VSIX files are not a license to
  imply native-installer trust guarantees we do not yet automate.

## Cross-Repo Testing

After changes to core crates, verify the downstream consumer:

```bash
# batchalign (Python + Rust)
cd /path/to/batchalign3
uv run maturin develop    # Rebuild Rust extension
uv run pytest             # 878 tests
```

CLI, LSP, and CLAN tests run as part of the main workspace's `bazel test //...` and `bazel build //... && bazel test //...`.
