# Architecture Overview

**Status:** Current
**Last updated:** 2026-05-11 08:29 EDT

The `talkbank-tools` repository holds the entire TalkBank toolchain: the
CHAT specification, tree-sitter grammar, parsing/model/validation/transform
crates, the `chatter` CLI, the LSP server, the VS Code extension, the
desktop app, and the Batchalign runtime/application layer. Everything lives
in one repository under one root Cargo workspace.

## Data Flow

Specification is the source of truth. Code is generated downstream from it.

```text
spec/           Source of truth (CHAT specification)
    ↓
grammar.js      Tree-sitter grammar (in grammar/)
    ↓
parser.c        Generated C parser (never hand-edited)
    ↓
Rust crates     Parser → Model → Validation → Transform
    ↓
Applications    chatter CLI, LSP server, VS Code, desktop app, batchalign
```

## Two layers

The repository contains two architectural layers:

**CHAT core (`talkbank-*` crates).** Parsing, data model, validation,
transform, CLAN analysis, CLI, LSP. Compiles and runs on a fresh machine
with no model downloads, no network, no Python. Everything that "is CHAT"
lives here. See the crate-boundary policy in `talkbank-tools/CLAUDE.md`
for what goes where.

**Batchalign runtime (`batchalign-*` crates + `batchalign/` Python).** ML
applications layered on top of the CHAT core: ASR, forced alignment,
morphosyntax, utterance segmentation, translation, coreference, audio
analysis. Rust owns all CHAT semantics; Python is a stateless ML inference
host. The standalone `batchalign3` repo was decommissioned in 2026-04;
its source folded in here as sibling crates.

## Crate Dependency Graph

```mermaid
flowchart TD
    derive["talkbank-derive\nProc macros"]
    model["talkbank-model\nData model, validation, alignment, errors"]
    parser["talkbank-parser\nCanonical parser (tree-sitter)"]
    re2c["talkbank-parser-re2c\nAlternate parser (equivalence oracle)"]
    transform["talkbank-transform\nPipelines, CHAT↔JSON, caching"]
    clan["clan-core\nCLAN analysis commands"]
    cli["chatter-cli (chatter)\nCLI: validate, normalize, convert"]
    lsp["chatter-lsp\nLanguage Server Protocol"]
    s2c["send2clan-sys\nFFI to CLAN app"]
    desktop["chatter-gui\nDesktop validation app (Tauri)"]
    tests["talkbank-parser-tests\nEquivalence tests"]

    batchalign_types["batchalign-types\nWire types (V2 protocol)"]
    batchalign["batchalign\nbatchalign3: server, runner, dispatch, workers, CLI binary"]
    batchalign_pyo3["batchalign-pyo3\nPython↔Rust worker runtime (.so, cdylib+rlib)"]

    derive --> model
    model --> parser & re2c
    parser --> transform
    transform --> clan & cli & lsp & desktop
    clan --> cli & lsp
    s2c --> cli & desktop
    parser --> tests
    re2c --> tests

    model --> batchalign
    parser --> batchalign
    transform --> batchalign & batchalign_pyo3
    batchalign_types --> batchalign & batchalign_pyo3
```

## Repository Layout

```text
talkbank-tools/
├── grammar/                Tree-sitter grammar
├── spec/                   CHAT specification (source of truth)
│   ├── constructs/         Valid CHAT examples + expected parse trees
│   ├── errors/             Invalid CHAT examples + expected error codes
│   ├── symbols/            Shared symbol registry (JSON)
│   ├── tools/              Core spec generators
│   └── runtime-tools/      Runtime-aware spec bootstrap/validation tools
├── crates/                 All Rust crates (CHAT core + batchalign)
├── corpus/                 Reference corpus
├── schema/                 JSON Schema (auto-generated)
├── apps/vscode-extension/                 VS Code extension
├── apps/chatter/chatter-gui/   Desktop validation app (Tauri v2, React)
├── apps/batchalign/dashboard-desktop/ Batchalign dashboard Tauri shell (experimental)
├── batchalign/             Python worker code (ML inference only)
├── apps/batchalign/cli-web-statuspage/               React dashboard (served by batchalign server)
├── book/                   This documentation
└── fuzz/                   Fuzz testing targets (separate Cargo workspace)
```

## Cargo Workspaces

Two separate workspaces:

1. **Root workspace** (`Cargo.toml`) — all Rust crates for parsing, model,
   transform, batchalign runtime, plus `apps/chatter/chatter-gui/src-tauri`.
2. **Spec workspace** (`crates/spec/talkbank-spec-testgen/Cargo.toml`) — `spec/tools` for core
   generation, `spec/runtime-tools` for runtime-aware spec tooling.

Use the relevant manifest path when working in the spec workspace:
`crates/spec/talkbank-spec-testgen/Cargo.toml` for generators, `crates/spec/talkbank-spec-testrun/Cargo.toml`
for bootstrap/mining/runtime validation.

## Where to read next

For per-topic detail (sections being consolidated; see SUMMARY for the
authoritative current list):

- [Spec System](spec-system.md), [Grammar](grammar.md),
  [Parser Backends](parser-backends.md) — how CHAT becomes typed AST.
- **CHAT model** — the AST itself, content traversal,
  [wide-struct rule](chat-model/wide-structs.md).
- [Alignment](alignment.md) — tier alignment, DP, forced alignment.
- **Runtime** — server, dispatch, concurrency, model loading,
  per-command data flow, caching.
- **Python–Rust boundary** — how Python workers fit in (batchalign only;
  the CHAT core has no Python).
- **Language and multilingual** —
  [Cantonese / CJK](language-and-multilingual/cantonese-and-cjk.md),
  language routing, Stanza.
- **Errors and validation** — per-app error systems, validation gates
  G0–G10.
- [Memory and Ownership](memory-and-ownership.md), Type-Driven Design
  (lands during M11 errors-and-validation work).
- [XML Emitter](xml-emitter.md) — projection.

For per-crate summaries see [Crate Reference](crate-reference.md).
