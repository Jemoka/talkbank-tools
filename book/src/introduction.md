# Introduction

**Status:** Current
**Last updated:** 2026-06-01 00:50 PDT

[TalkBank](https://talkbank.org/) is the world's largest open repository of spoken language data. This repository (`talkbank-tools`) is the home of several related surfaces: the `chatter` CLI, source-first preview Rust crates for CHAT parsing/validation, the `tree-sitter-talkbank` grammar, the `chatter-lsp` language server, the VS Code extension, and the preview `batchalign3` product line.

Platform and support status now depend on the surface. `chatter` ships release binaries for Windows, macOS, and Linux; the public Rust core is a source-level cross-platform surface; `batchalign3` is public preview on the same three operating systems; and both desktop apps in this repo remain experimental. See the repo-root `book/src/operations/platform-support.md` and `book/src/operations/release-contract.md`.

## Choose the right surface

| Task | Recommended Surface | Support Status |
|---|---|---|
| **CHAT validation, normalization, conversion, or CLAN analysis** | `chatter` CLI | ✅ Stable; binaries for Windows, macOS, Linux |
| **Edit CHAT files with live validation** | VS Code extension + `chatter-lsp` | 🔷 Preview; VSIX bundles from GitHub Releases |
| **Transcribe, align, or morphotag CHAT with audio/ML** | `batchalign3` CLI/server | 🔷 Preview; wheels for Windows, macOS, Linux |
| **LSP integration in other editors** | `chatter-lsp` standalone | 🔷 Preview; Neovim, Emacs, Helix, Zed compatible |
| **Build CHAT tooling in Rust** | Public Rust crates (`talkbank-model`, `talkbank-parser`, etc.) | 🔷 Preview; source-first via path dependencies |
| **Reuse grammar in other tools** | `tree-sitter-talkbank` npm package | 🔷 Preview; API not yet frozen |
| **Standalone desktop GUI for CHAT validation** | Chatter Desktop (`apps/chatter/chatter-gui/`) | ⚠️ Experimental only; build from source |
| **Standalone desktop GUI for Batchalign** | Batchalign Desktop (`apps/batchalign/batchalign-gui/`) | ⚠️ Experimental only; build from source |

**Legend:** ✅ = Stable public release | 🔷 = Public preview | ⚠️ = Experimental (not supported for end-users)

## What's In This Repo

- **`chatter` CLI** — validate, convert, normalize, and analyze CHAT files from the command line, with an interactive TUI for corpus-scale workflows
- **VS Code extension** — live validation, cross-tier alignment visualization, media playback, transcription mode, dependency graphs, and access to the CLAN command surface from the editor
- **Language Server (LSP)** — powers VS Code but works with any LSP-compatible editor (Neovim, Emacs, Helix, Zed, etc.)
- **CLAN command surface** — 1 validation + 34 analysis + 23 transform commands plus a handful of format converters, covering the full CLAN binary set in `OSX-CLAN/src/unix/bin/`. Six legacy NLP commands (MOR, POST, MEGRASP, etc.) are deliberately not implemented and direct callers to Batchalign's neural pipeline instead. See the [CLAN command status matrix](clan-reference/appendices/status-matrix.md).
- **JSON data model** — every CHAT structure as typed JSON with lossless roundtrip fidelity, backed by a published JSON Schema
- **Rust API** — parse, validate, inspect, and transform CHAT files programmatically via library crates

## Who This Book Is For

- **Researchers** who validate or analyze CHAT files → [Installation](chatter/user-guide/installation.md), [CLI Reference](chatter/user-guide/cli-reference.md)
- **CLAN users** migrating to the new toolchain → [Migration Guide](chatter/user-guide/migrating-from-clan.md)
- **Editors** using VS Code for transcription → [VS Code Extension](chatter/user-guide/vscode-extension.md)
- **Developers** building on the CHAT format → [CHAT Format](chat-format/overview.md)
- **Integrators** consuming CHAT data via JSON or Rust → [Integration Guide](chatter/integrating/library-usage.md)
- **Contributors** to the toolchain itself → [Contributing](contributing/setup.md)

## Repository Layout

```text
grammar/                  Tree-sitter grammar (~380 rules, ~410 node types)
resources/spec/           Source of truth: CHAT specification + error specs
resources/corpus/         Reference corpus (.cha files, 100% pass required)
crates/                   Rust crates, grouped by product
  crates/core/              talkbank-model, talkbank-parser, talkbank-transform, etc.
  crates/chatter/           chatter-cli, chatter-lsp
  crates/clan/              clan-core, send2clan-sys
  crates/batchalign/        batchalign-core, batchalign-engine (and others)
  crates/spec/              spec tooling
python/                   batchalign3 Python package (ML inference hosting)
apps/                     Tauri v2 desktop apps and VS Code extension
  apps/chatter/             Chatter Desktop (experimental)
  apps/batchalign/          Batchalign Desktop + dashboard (experimental)
  apps/vscode-extension/    VS Code extension (TypeScript)
schemas/                  JSON Schemas (chat-file, ipc)
fuzz/                     Fuzz testing targets (separate Cargo workspace)
book/                     This documentation (mdBook)
```

Data flows: **spec** (source of truth) → **grammar** (tree-sitter) → **Rust crates** (parsers, model, validation, CLAN, CLI, LSP) → **applications** (chatter, batchalign3, VS Code, desktop apps).
