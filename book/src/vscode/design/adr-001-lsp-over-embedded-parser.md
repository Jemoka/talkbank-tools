# ADR-001: LSP over Embedded Parser

**Status:** Accepted
**Last updated:** 2026-04-16 22:00 EDT

## Context

The extension needs to parse and validate CHAT text on every
keystroke, resolve alignment metadata for hover / highlights / graph
features, and run CLAN analyses on demand. Two plausible shapes:

1. **Embedded parser.** Ship a WASM or npm build of
   `tree-sitter-talkbank` plus a TypeScript port of
   `talkbank-model` / `talkbank-validation` so the extension can
   parse CHAT in-process.
2. **Language server.** Ship a Rust binary (`chatter-lsp`) that
   hosts the entire parser + model + validation + CLAN analysis
   stack and talks to the extension over LSP stdio.

## Decision

**Use a language server.** The extension is a thin
presentation/workflow layer; `chatter-lsp` owns every piece of
CHAT domain knowledge.

Concretely:

- The LSP binary is built from the same Rust workspace that produces
  `chatter` (the CLI) and is used by `batchalign3`. Embedding would
  fork the codebase into two parallel implementations.
- CHAT parsing goes through `tree-sitter-talkbank` → `talkbank-parser`
  → `talkbank-model::ChatFile`, with incremental reparse driven by
  tree-sitter edits.
- All alignment, validation, and CLAN commands run in the server.
- Custom features that don't fit standard LSP (e.g. dependency graph
  DOT, alignment sidecar, speaker extraction) expose themselves
  through twelve `talkbank/*` custom `executeCommand` endpoints —
  see [reference/rpc-contracts.md](../reference/rpc-contracts.md).

## Consequences

**Positive.**

- One parser implementation, one validation implementation, one CLAN
  implementation. No TS/Rust drift.
- The same Rust model that powers the VS Code extension also powers
  `chatter validate`, `batchalign3`, the Tauri desktop app. Bug fixes
  land in one place.
- Rich incremental parsing via tree-sitter without porting the C
  grammar to JavaScript.
- Memory-intensive structures (validation caches, tree-sitter trees)
  live in the server's address space, not VS Code's — and die with
  the server process when the extension unloads.

**Negative.**

- The VSIX must ship a platform-specific binary for each target OS
  (macOS ARM + Intel, Linux x86 + ARM, Windows x86). See
  [ADR-004](adr-004-bundled-lsp-binary.md).
- First-time activation fails on platforms without a prebuilt
  binary unless the user sets
  `talkbank.lsp.binaryPath` to a local `cargo install` location.
- LSP transport adds a JSON-RPC roundtrip to every feature request.
  Latency measured in tens of microseconds — insignificant vs. the
  human-scale interactions it supports.
- Protocol design discipline required: every new feature either fits
  a standard LSP request or gets a typed custom `talkbank/*`
  endpoint. No shared in-memory state across the TS↔Rust boundary.

## Alternatives considered

**Embed tree-sitter-talkbank only, validate in TS.** Rejected: the
validation codebase is 3,500+ lines of Rust with deep coupling to
the parser's CST shape and the typed `Mor` / `Gra` / `Pho` /
`Sin` / `Wor` / `ChatFile` models. Porting to TypeScript would
create a permanent maintenance tax.

**Embed a WASM build of the entire Rust stack.** Rejected at the
time for toolchain maturity and memory-footprint reasons; may be
worth revisiting when `wasm-bindgen` + async LSP-over-WASM mature
(the MCP / skills ecosystem is pushing adjacent patterns). If
revisited, it would be a transport swap, not a re-architecture —
the server code would stay the same.

**Skip a server and ask the CHAT tree-sitter grammar to do enough
syntax highlighting alone.** Rejected: the extension's flagship
features (cross-tier alignment, dependency graph, CLAN analyses,
validation diagnostics) all need the full typed model, not just
CST node ranges.

## Source anchors

- Extension-side LSP activation: `src/activation/lsp.ts`.
- Server binary target: `crates/chatter-lsp/src/bin/chatter-lsp.rs`.
- Server dispatch entry: `crates/chatter-lsp/src/backend/mod.rs`.
- Custom RPC registration: `crates/chatter-lsp/src/backend/execute_commands.rs`.
