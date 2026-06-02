# Release Contract

**Status:** Current
**Last updated:** 2026-06-01 00:50 PDT

This document defines which surfaces in the merged `talkbank-tools` repository
are part of the public release contract. A version number in a manifest,
generated artifact, or runtime banner does **not** by itself mean the surface is
public or stability-guaranteed.

## Tier definitions

| Tier | Meaning |
|------|---------|
| **Public stable** | Publicly documented surface with an explicit compatibility promise. For the current pre-1.0 TalkBank core, semver expectations apply, but breaking changes may still happen in minor releases and must be called out clearly. |
| **Public preview** | Publicly installable or documented surface that we expect users to try, but we are not yet promising minor-version compatibility. |
| **Experimental** | Present in the repo, may build, but not part of the supported public release contract. |
| **Internal** | Implementation surface only. Names, versions, wire formats, and file layout may change without notice. |

## Current surface classification

| Surface | Tier | Canonical version source | Notes |
|---------|------|--------------------------|-------|
| `chatter` CLI | **Public stable** | `Cargo.toml` `workspace.package.version` | User-facing CLI contract for CHAT validation, normalization, conversion, and CLAN-compatible commands. |
| Public TalkBank Rust crates: `talkbank-model`, `talkbank-parser`, `talkbank-transform`, `clan-core` | **Public preview** | `Cargo.toml` `workspace.package.version` | Documented source-first Rust dependency surfaces. Today consumers use git/path dependencies: the release workflow is binary-only, there is no dedicated crates.io publish workflow yet, and the current manifests still rely on unpublished helper crates such as `talkbank-derive` and `talkbank-re2c-parser`. |
| `tree-sitter-talkbank` standalone grammar packages | **Public preview** | `grammar/Cargo.toml` `package.version` | One grammar release line mirrored into npm and PyPI metadata. The grammar is public and reusable, but CST/API shape may still evolve before stabilization. |
| `batchalign3` Python package, bundled CLI, and user-visible Batchalign version banners | **Public preview** | `pyproject.toml` `[project].version` | Publicly installable and documented on the `0.1.x` preview line. Internal Cargo manifests mirror this version so the CLI/server/runtime surface presents one product release line, but the compatibility promise remains preview-tier. |
| `batchalign3 serve` local server and dashboard UI | **Public preview** | Tracks the `batchalign3` product release line in `pyproject.toml` | End-user surface is supported, but the REST/WebSocket/API contract is not yet frozen for third-party integrations. |
| `chatter-lsp` language server | **Public preview** | `Cargo.toml` `workspace.package.version` | Functional and documented, but protocol/configuration details may still evolve. |
| VS Code extension | **Public preview** | `apps/vscode-extension/package.json` `version` | First public release channel is GitHub Releases VSIX-only. Marketplace publishing is intentionally deferred while binary discovery and release ops are still being hardened. |
| Desktop shells in `apps/chatter/chatter-gui/` and `apps/batchalign/batchalign-gui/` | **Experimental** | Their local `package.json` / `Cargo.toml` manifests | In-repo experiments only; not part of the supported release contract. |
| Internal implementation surfaces: `crates/batchalign-*` Rust crates, `batchalign-pyo3`, `send2clan-sys`, `apps/batchalign/cli-web-statuspage/`, generated worker/OpenAPI artifacts | **Internal** | Local manifests and generated files | Build/runtime internals only. The Batchalign Rust/PyO3 crates are explicitly unpublished (`publish = false`), so matching the public product version does **not** make them supported public Rust APIs. External consumers should not depend on them directly. |

## Conservative promises

- Only the TalkBank core CLI is currently treated as a stable public surface.
- The four listed TalkBank Rust crates are documented and semver-checked, but
  they remain **public preview** until the repo has a real crates.io
  publication path rather than only source-level git/path consumption.
- `tree-sitter-talkbank` is releasable and public, but it remains preview-tier
  until the standalone grammar API is explicitly frozen.
- `batchalign3` is now a major top-level product in this merged repository, but
  it is still classified as **public preview** at the repo-contract level on a
  deliberately pre-1.0 release line. Its package version does not imply that
  every Batchalign-adjacent sub-surface is stable.
- Preview, experimental, and internal surfaces may change independently even
  when their version numbers happen to match a public surface. For Batchalign,
  matching Cargo-manifest versions are a packaging-coherence choice, not a
  crates.io publication promise.
- Preview and experimental surfaces should remain below `1.0.0` until this
  document and `book/src/operations/versioning.md` are updated to grant a stronger promise.

## What this document does not promise

- Matching version numbers across Rust, Python, VS Code, and desktop surfaces.
- Stable third-party integration against the Batchalign REST/WebSocket/API
  layer.
- A first-release crates.io publication promise for the TalkBank Rust library
  crates.
- Stability for repo-internal crates, generated schemas, or FFI support code.

## Related policies

- `book/src/operations/versioning.md` defines the canonical version source for each surface.
- `book/src/operations/platform-support.md` defines platform tiers separately from stability
  tiers.
- Promoting any surface to **Public stable** requires updating this document,
  `book/src/operations/versioning.md`, and the corresponding CI/release automation together.

## License

BSD-3-Clause across all release surfaces.
