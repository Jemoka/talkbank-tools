# Versioning Policy

**Status:** Current
**Last updated:** 2026-04-29 10:26 EDT

This repository ships multiple product surfaces with different audiences and
different release mechanics. A version field is only authoritative for the
surface it is assigned to. If several files contain versions, the canonical
source below wins for public communication and release policy.

## Canonical version sources

| Surface / release line | Release tier | Canonical source | Current value | Notes |
|------------------------|--------------|------------------|---------------|-------|
| TalkBank core Rust release line (`chatter`, preview TalkBank crates, preview `chatter-lsp`) | Mixed: stable CLI + preview libraries/LSP | `Cargo.toml` `workspace.package.version` | `0.2.0` | Shared version for the main Rust workspace. Matching the workspace version does not imply that every Rust crate already has a crates.io publication path. |
| Standalone `tree-sitter-talkbank` grammar packages (crates.io, npm, PyPI) | Public preview | `grammar/Cargo.toml` `package.version` | `0.2.0` | `grammar/package.json` and `grammar/pyproject.toml` mirror this value for the same grammar release line. |
| `batchalign3` public product release line (Python package, bundled CLI, local server, dashboard UI) | Public preview | `pyproject.toml` `[project].version` | `0.1.0` | Authoritative version for PyPI/wheel installs and the user-visible Batchalign product surface. |
| Batchalign runtime banner metadata | Internal | `batchalign/version` | `0.1.0` | Runtime metadata only; intentionally mirrors the public Batchalign preview version, but it is not a release-contract source. |
| Batchalign Rust/PyO3 build crates (`crates/batchalign-*`, `pyo3/`) | Internal | Their local `Cargo.toml` manifests, kept in lockstep with `pyproject.toml` | `0.1.0` | Internal build metadata only. These crates are unpublished (`publish = false`) and are not independent public Rust APIs. |
| VS Code extension | Public preview | `apps/vscode-extension/package.json` `version` | `0.1.0` | Independent Marketplace release line. |
| Experimental desktop shells | Experimental | `desktop/package.json`, `desktop/src-tauri/Cargo.toml`, `apps/batchalign/dashboard-desktop/package.json`, `apps/batchalign/dashboard-desktop/src-tauri/Cargo.toml` | `0.1.0` | Experimental only; not part of the public release contract. |

## Rules

1. **One public surface, one canonical version source.** Public docs, release notes, and support answers should cite the canonical source for that surface, not whichever version field happens to be easiest to find.

2. **Preview and experimental surfaces stay below `1.0.0` until explicitly promoted.** A `1.0.0` tag is a stability signal, not just a bigger number. Do not ship it for preview or experimental surfaces unless `book/src/operations/release-contract.md`, this file, and the release workflow/docs are all updated together to describe the stronger promise.

3. **TalkBank core Rust surfaces share one version.** `Cargo.toml` `workspace.package.version` is the source of truth for `chatter`, the preview TalkBank library crates, and preview `chatter-lsp`.

4. **The standalone grammar ships as one cross-ecosystem release line.** `grammar/Cargo.toml` is the canonical version source for `tree-sitter-talkbank`; the npm and PyPI metadata must mirror it in the same patch so crates.io, npm, and PyPI do not drift.

5. **`batchalign3` public versioning is Python-package-first.** `pyproject.toml` defines the public Batchalign release line. The Batchalign Cargo manifests mirror that version for packaging and operator-facing coherence, but the public contract still comes from the Python package surface, not from unpublished Rust crates.

6. **`batchalign/version` is not authoritative for release policy.** It is runtime metadata only. Do not use it to infer what the public Batchalign version or compatibility promise is.

7. **Matching versions do not widen the contract.** The Batchalign Cargo manifests intentionally mirror the public `batchalign3` release line, but `publish = false` keeps those crates internal-only. Equal version numbers do not create a crates.io support promise or a public Rust semver contract.

8. **CI semver checks are a Rust guardrail, not a blanket publication or crates.io promise.** `cargo-semver-checks` runs in CI, but only the surfaces marked public/stable in `book/src/operations/release-contract.md` are currently covered by the repo's strongest external compatibility promise.

9. **The TalkBank library crates are source-first until publication work exists.** The current release workflow ships `chatter` and preview `chatter-lsp` binaries, while library consumers still use git/path dependencies. Do not describe `workspace.package.version` as a crates.io-ready release line for `talkbank-model`, `talkbank-parser`, `talkbank-transform`, or `clan-core` until a dedicated publish workflow and dependency-ready manifests exist.

10. **The VS Code extension and desktop apps version independently.** Their versions do not need to match the Rust workspace, the grammar line, or the Batchalign Python package.

11. **If a surface changes tier or version authority, update policy docs together.** `book/src/operations/release-contract.md` and `book/src/operations/versioning.md` should change in the same patch.

12. **When Batchalign ships a new public product version, mirror it into the Batchalign Cargo manifests in the same patch.** Update `pyproject.toml` first, then the `crates/batchalign-*/Cargo.toml`, `pyo3/Cargo.toml`, and any user-visible runtime metadata that intentionally echoes the product release line.

## Pre-1.0 Policy

Every currently releasable surface in this repo is still pre-1.0.

- **Patch (`0.x.y`)**: bug fixes, packaging metadata corrections, documentation-only
  release metadata updates, and other changes that should not require users to
  rethink integrations.
- **Minor (`0.y.0`)**: any user-visible feature addition, behavior shift, or
  breaking change. For pre-1.0 surfaces, the minor bump is the compatibility
  signal that would be a major bump after stabilization.
- **Promotion to `1.0.0`**: requires an explicit release-contract decision for
  that surface plus coordinated updates to its workflow/docs. Do not infer
  readiness from accumulated features alone.

For the current TalkBank core stable line, breaking changes may still occur in
minor versions and must be called out clearly.

After a surface is explicitly promoted to stable at `1.0.0` or later:
- **Patch:** bug fixes only
- **Minor:** backwards-compatible additions
- **Major:** breaking changes (with deprecation period)

## Surface-aware version bumps

1. Identify which public surface is changing.
2. Update that surface's canonical version source first.
3. Update any secondary packaging metadata that must match for build/distribution
   (for example, grammar npm/PyPI mirrors or Batchalign internal manifests).
4. Re-run the relevant checks for that surface (for example, Rust semver checks
   for public Rust APIs, or preview release smoke tests for Batchalign/VS Code).
5. If the change also promotes or demotes a surface, update
   `book/src/operations/release-contract.md` in the same patch.
