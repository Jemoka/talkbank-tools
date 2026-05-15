# Repository Architecture and Boundaries

**Status:** Current
**Last updated:** 2026-05-01 05:30 EDT

## Top-level layout

```text
spec/                     canonical syntax and error spec source
spec/tools/               deterministic generators + validators (separate Cargo workspace)
grammar/                  tree-sitter grammar source + generated parser artifacts
crates/                   all Rust crates (root Cargo workspace)
  talkbank-model/         data model, validation, alignment, errors, parser API trait
  talkbank-derive/        proc macros (SemanticEq, SpanShift, ValidationTagged, error_code_enum)
  talkbank-parser/        canonical parser (tree-sitter)
  talkbank-parser-re2c/   alternate parser (specification oracle, opt-in batch parser)
  talkbank-parser-tests/  parser equivalence and roundtrip tests
  talkbank-transform/     pipelines, CHAT↔JSON, caching, parallel validation
  clan-core/          CLAN analysis commands and format converters
  chatter-cli/           the `chatter` CLI binary
  chatter-lsp/           LSP server
  send2clan-sys/          C FFI to the legacy CLAN app
  batchalign/             Batchalign runtime: CLI, axum server, dispatch, FA, morphosyntax, Rev.AI client
  batchalign-types/       Batchalign shared domain + worker IPC types
  batchalign-pyo3/        PyO3 bridge — builds the `batchalign_core` Python extension
batchalign/               Batchalign Python worker code (ML inference hosting only)
apps/                     desktop apps (Tauri v2 + React): chatter-gui, dashboard-desktop
apps/batchalign/cli-web-statuspage/                 React dashboard for the Batchalign server
apps/vscode-extension/                   VS Code extension (TypeScript)
corpus/                   reference corpus (must pass 100%)
schema/                   JSON Schema for ChatFile AST
tests/                    workspace-level integration tests and fixtures
fuzz/                     fuzz targets (separate Cargo workspace)
book/                     mdBook documentation source
docs/                     release-contract policy documents and the auto-generated error catalog
```

## Architectural principles

1. Clear boundaries between specification, generation, runtime
   logic, and documentation.
2. Generated artifacts and hand-authored code are kept separate with
   hard guardrails — `parser.c`, `node-types.json`, generated tests
   and error-doc artifacts are never edited by hand.
3. Each crate has a single clear responsibility.
4. Entry-point docs guide new contributors to authoritative
   references quickly.

## Canonical ownership rules

- `spec/` owns the language intent and accepted examples — what
  CHAT *means*.
- `grammar/` owns tokenization and CST shape only, not semantic
  validation policy.
- `talkbank-model` owns semantic validity, serialization
  invariants, error types, and parser API contracts.
- `talkbank-transform` owns pipelines and JSON schema validation.
- `batchalign` (the crate) owns the Batchalign-specific runtime,
  server, and ML-pipeline orchestration; it consumes the talkbank-*
  crates above.

## Dependency direction rules

1. `spec` does not depend on runtime crates.
2. `grammar` is consumed by parser crates, not vice versa.
3. `talkbank-model` is dependency-minimal and stable; all other
   talkbank-* crates depend on it.
4. The `batchalign` crate consumes the talkbank-* core crates
   (model, parser, transform, etc.) but never reimplements CHAT
   primitives.
5. CLI / LSP / desktop apps depend on stable internal APIs, never
   directly on unstable internals of other crates.
6. Generator tools may read specs and grammar metadata but do not
   become runtime dependencies.

## Acceptance criteria

- Every top-level directory has a clear purpose statement.
- No crate depends on internal modules outside declared boundaries.
- No generated artifact is edited manually.
- New contributors can identify authoritative docs in less than
  five minutes.
