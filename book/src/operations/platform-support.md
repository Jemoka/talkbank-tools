# Platform Support Matrix

**Status:** Current
**Last updated:** 2026-06-01 00:50 PDT

This document answers two separate questions for each public-facing surface in
`talkbank-tools`:

1. **Is this a supported public surface?** See also
   [`release-contract.md`](release-contract.md).
2. **What platform claim is accurate today?** That includes CI coverage,
   release artifacts, and important caveats.

## Quick surface summary

| Surface | Release/support status | Accurate platform claim today |
|---|---|---|
| `chatter` CLI | Public stable | Release binaries for Linux x86_64, macOS arm64/x86_64, and Windows x86_64 |
| Public Rust core crates (`talkbank-model`, `talkbank-parser`, `talkbank-transform`, `clan-core`) | Public preview | Source-first cross-platform surface; Linux is the only full PR test gate today |
| `tree-sitter-talkbank` grammar | Public preview | Reusable cross-platform grammar package; grammar CI currently runs on Ubuntu |
| `chatter-lsp` | Public preview | Bundled in the same core release archives as `chatter` for Linux x86_64, macOS arm64/x86_64, and Windows x86_64 |
| VS Code extension | Public preview | GitHub Releases publish platform-specific VSIX bundles (macOS arm64/x64, Linux x64/arm64, Windows x64) |
| `batchalign3` CLI / local server / dashboard | Public preview | Wheels for macOS arm64/x86_64, Linux x86_64/aarch64, and Windows x86_64 |
| Chatter Desktop (`apps/chatter/chatter-gui/`) | Experimental | In-repo validation GUI only; no supported release distribution |
| Batchalign Desktop (`apps/batchalign/batchalign-gui/`) | Experimental | In-repo Batchalign GUI shell only; no supported release distribution |

## Support level definitions

- **Tier A:** Full PR CI gate for that surface on the named platform.
- **Tier B:** Public release artifact or explicit smoke coverage exists, but not a
  full PR test gate.
- **Tier C:** Experimental/in-repo only. No supported end-user guarantee.

## `chatter` CLI

| Platform | Tier | CI / artifact evidence | Notes |
|---|---|---|---|
| Linux x86_64 | A | PR CI runs tests and `chatter` smoke; release archives built | Strongest current support story |
| macOS arm64 | B | Cross-platform smoke in PR CI; release archive built | Native smoke coverage exists, but not a full test matrix |
| macOS x86_64 | B | Release archive built | Build coverage only |
| Windows x86_64 | B | Cross-platform smoke in PR CI; release archive built | Smoke coverage exists, but not a full test matrix |

## Public Rust core crates

These are the documented source-first Rust dependency surfaces today:
`talkbank-model`, `talkbank-parser`, `talkbank-transform`, and `clan-core`.

| Platform | Tier | CI / artifact evidence | Notes |
|---|---|---|---|
| Linux x86_64 | A | PR CI runs crate tests directly | Canonical source-level gate |
| macOS arm64 | B | Compiled transitively in core release builds | No crate-level macOS test job yet |
| macOS x86_64 | B | Compiled transitively in core release builds | No crate-level Intel macOS test job yet |
| Windows x86_64 | B | Compiled transitively in core release builds | No crate-level Windows test job yet |

These crates are supported as **source** surfaces, not as separate prebuilt
downloads or published crates.io installs today.

## `tree-sitter-talkbank` grammar

| Aspect | Tier | Current claim |
|---|---|---|
| Grammar source + generated artifacts | A on Linux CI, B elsewhere | Grammar generation and `tree-sitter test` run in Ubuntu CI |
| Standalone grammar packages | B | Public preview reusable package line (`npm`, `crates.io`, and PyPI metadata) |
| Downstream bindings | B for Rust, C for others | Rust is the primary maintained binding; other bindings in `bindings/` are still scaffold-level |

Use cross-platform language here carefully: the grammar itself is intended to be
portable, but the CI proof today is Ubuntu-only and the binding story is not
uniform across languages.

## `chatter-lsp`

| Platform | Tier | CI / artifact evidence | Notes |
|---|---|---|---|
| Linux x86_64 | B | Bundled in core release archives; VSIX packaging smoke stages the binary | No standalone PR test job for `chatter-lsp` on Linux |
| macOS arm64 | B | Bundled in core release archives | Ships via release archives and VSIX packaging |
| macOS x86_64 | B | Bundled in core release archives | Ships via release archives and VSIX packaging |
| Windows x86_64 | B | Bundled in core release archives | Ships via release archives and VSIX packaging |

`chatter-lsp` is public preview: usable and distributed, but not yet described
as a fully frozen integration contract.

## VS Code extension

| Platform | Tier | CI / artifact evidence | Notes |
|---|---|---|---|
| Linux x64 VSIX | B | PR CI compiles/tests extension and smokes Linux VSIX packaging | Strongest packaging coverage today |
| Linux arm64 VSIX | B | Release workflow builds VSIX | Release-built only |
| macOS arm64 VSIX | B | Release workflow builds VSIX | Release-built only |
| macOS x64 VSIX | B | Release workflow builds VSIX | Release-built only |
| Windows x64 VSIX | B | Release workflow builds VSIX | Release-built only |

For now, user-facing distribution should be described as **GitHub Releases
VSIX-only**, not Marketplace-first.

## `batchalign3` CLI / local server / dashboard

| Platform | Tier | CI / artifact evidence | Notes |
|---|---|---|---|
| Linux x86_64 | A | Wheel smoke + PR CI on Ubuntu | Primary Batchalign CI platform |
| Linux aarch64 | B | Release wheel built | Build coverage only |
| macOS arm64 | B | Wheel smoke + release wheel | Stronger than build-only, but not a full PR suite |
| macOS x86_64 | B | Release wheel built | Build coverage only |
| Windows x86_64 | B | Wheel smoke + release wheel | CLI install/help path is covered; server/worker mode still has limitations |
| Dashboard UI (browser) | B | Built in CI; desktop/web frontend shared | Supported as part of the preview Batchalign surface, but the API is not frozen for third parties |

Important Windows caveat: server/worker lifecycle code still has Unix-specific
gaps, so avoid describing Windows as feature-identical to Linux/macOS for every
Batchalign deployment mode.

## Desktop surfaces

The repo contains **two different desktop apps**. They are easy to confuse, so
docs should always name them explicitly:

- **Chatter Desktop** = `apps/chatter/chatter-gui/` = experimental native GUI for CHAT
  validation-only workflows.
- **Batchalign Desktop** = `apps/batchalign/batchalign-gui/` = experimental native shell
  around the Batchalign processing/dashboard UI.

### Chatter Desktop (`apps/chatter/chatter-gui/`)

| Platform | Tier | Current claim |
|---|---|---|
| macOS / Windows / Linux | C | Experimental in-repo app only; not part of the supported public release contract |

### Batchalign Desktop (`apps/batchalign/batchalign-gui/`)

| Platform | Tier | Current claim |
|---|---|---|
| macOS / Windows | C | Experimental artifact workflow exists, but this is not a supported end-user release line |
| Linux | C | No supported public desktop distribution |

## How to phrase support publicly

- Say **"`chatter` is the stable public CHAT-first CLI"**.
- Say **"`batchalign3` is the public preview audio/ML surface"**.
- Say **"the Rust crates are public preview and currently source-first via
  git/path dependencies"**.
- Say **"the VS Code extension is public preview and currently distributed as
  GitHub Releases VSIX bundles"**.
- Say **"`tree-sitter-talkbank` is public preview"**, not "fully stable across
  all bindings".
- Say **"Chatter Desktop"** or **"Batchalign Desktop"** by name.
- Do **not** describe either desktop app as a generally supported end-user
  release surface today.
