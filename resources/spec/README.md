# `resources/spec/` — CHAT Format Specification (source of truth)

**Last updated:** 2026-05-15 (Phase 5 reorg)

This directory is the **single source of truth** for grammar tests, parser
tests, and error documentation. It is not user-facing documentation — for
that, see `book/`.

`just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` reads this directory and regenerates:

- `grammar/test/corpus/*.txt` — tree-sitter corpus tests
- `crates/core/talkbank-parser-tests/tests/generated/*.rs` — Rust parser/validator tests (canonical path)
- `book/src/operations/errors/*.md` — per-error documentation pages

**Never hand-edit those output files.** They are wiped and regenerated on every
`just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` run.

## Contents

| Subdir | Purpose |
|---|---|
| `constructs/` | Valid CHAT examples with expected parse trees. ~164 specs across `header/`, `main_tier/`, `tiers/`, `utterance/`, `word/`. |
| `errors/` | Invalid CHAT examples with expected error codes. ~197 spec files covering all 181 error codes. |
| `symbols/` | Shared symbol registry (`symbol_registry.json`) — language codes, error markers, etc. Generated into both `grammar/grammar.js` and Rust source via `node resources/spec/symbols/validate_symbol_registry.js && node scripts/generate-symbol-sets.js && node resources/spec/symbols/generate_rust_symbol_sets.js`. |
| `docs/` | Methodology notes about spec format, conventions, and the test-gen pipeline (`ERROR_SPEC_FORMAT.md`, `WRITING_ERROR_SPECS.md`, `CURATION_WORKFLOW.md`). |

## Generator crates

The generators that consume this directory live in the main Rust workspace
under `crates/spec/`:

| Crate | Role |
|---|---|
| `talkbank-spec-testgen` | **Static codegen.** Reads spec markdown, writes tree-sitter corpus tests, Rust parser/validator tests, and error docs. No dependency on the live parser. Powers `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`. 12 binaries (`gen_tree_sitter_tests`, `gen_rust_tests`, `gen_validation_tests`, `gen_error_docs`, `validate_spec`, etc.). |
| `talkbank-spec-testrun` | **Live-parser verification.** Runs each error spec through the actual `talkbank-parser` to confirm it produces the claimed error codes (`validate_error_specs`), and mines real `.cha` files for candidate specs (`extract_corpus_candidates`). |

Before the 2026-05-15 reorganization, both crates lived inside `spec/` as a
separate Cargo workspace. They now live under `crates/spec/` as ordinary
workspace members. The full workflow is documented in [`CLAUDE.md`](./CLAUDE.md).
