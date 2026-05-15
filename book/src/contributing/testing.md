# Testing

**Status:** Current
**Last updated:** 2026-04-29 10:39 EDT

## Test Generation Pipeline

Specs are the source of truth. All grammar corpus tests, Rust parser tests,
and error docs are **generated** from specs. `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` wipes the output
directories and recreates them — never hand-edit generated files.

```mermaid
flowchart LR
    subgraph sources["Source of Truth"]
        constructs["resources/spec/constructs/\n(112 construct specs)"]
        errors["resources/spec/errors/\n(187 error specs)"]
        templates["crates/spec/talkbank-spec-testgen/templates/\n(Tera wrappers)"]
    end

    subgraph generators["just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs"]
        gen_ts["gen_tree_sitter_tests"]
        gen_rust["gen_rust_tests"]
        gen_docs["gen_error_docs"]
    end

    subgraph outputs["Generated Outputs (DO NOT EDIT)"]
        ts_tests["grammar/test/corpus/\n(166 tree-sitter tests)"]
        rust_tests["tests/generated/\n(167 Rust tests)"]
        error_docs["book/src/operations/errors/\n(182 error doc pages)"]
    end

    constructs & errors --> gen_ts
    templates --> gen_ts
    constructs & errors --> gen_rust
    errors --> gen_docs

    gen_ts --> ts_tests
    gen_rust --> rust_tests
    gen_docs --> error_docs
```

To add a grammar test or error test, add a spec file in `resources/spec/constructs/`
or `resources/spec/errors/`, then run `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`. See `resources/spec/CLAUDE.md` for the
spec format.

## Test Strategy

Testing is organized in layers, from fastest to most comprehensive.

```mermaid
flowchart TD
    unit["Unit + Integration Tests\n(cargo nextest run)\n~2300 tests, ~5s"]
    specgen["Spec-Generated Tests\n(bazel test //crates/core/talkbank-parser-tests/...)\nParser + validation layer"]
    grammar["Grammar Corpus\n(tree-sitter test)\n166 tree-sitter tests"]
    ref["Reference Corpus\n(97 files, 100% required)"]
    gates["Verification Gates\n(bazel build //... && bazel test //...)\nG0–G14 sequential pipeline"]

    unit --> specgen --> grammar --> ref --> gates
```

### Unit Tests (nextest)

```bash
cargo nextest run
```

Runs all unit and integration tests across all crates (~2300+ tests). These test individual functions, serialization roundtrips, and model invariants.

`cargo nextest` does not run doctests. Keep `cargo test --doc` as a separate
verification step when you change public API examples or doc comments.

### Parser Equivalence

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
```

Runs the parser on each file in the 78-file reference corpus and validates
results. Each `.cha` file is its own test, enabling per-file parallelism and
failure isolation via nextest.

### Spec-Generated Tests

Part of `talkbank-parser-tests`. These are generated from specs via `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` and currently test:
- Construct specs: input parses correctly
- Parser-layer error specs: input fails to parse with expected error code
- Validation-layer error specs: input parses but validation reports expected error code

Concrete entrypoint:

```bash
bazel test //crates/core/talkbank-parser-tests/...
```

### Tree-Sitter Grammar Tests

```bash
bazel test //grammar/...
```

Runs the tree-sitter grammar corpus tests. This is the right gate for
grammar structure changes.

### Error Corpus Tests

Supplementary test fixtures in `tests/error_corpus/`. The `expectations.json` manifest maps `.cha` files to expected outcomes.

### Tree-Sitter Tests

```bash
cd grammar
tree-sitter test
```

160+ tests that verify the grammar produces correct CSTs for known inputs.

## Reference Corpus

The reference corpus at `resources/corpus/reference/` contains 97 `.cha` files across 20 languages that represent the diversity of real-world CHAT data. The parser must handle every file at 100%.

This corpus is the ultimate arbiter of correctness for full-file parsing.

## Verification Gates

`bazel build //... && bazel test //...` runs the pre-merge verification suite (G0–G14). All gates
run sequentially — the first failure stops the pipeline.

```mermaid
flowchart TD
    start(["bazel build //... && bazel test //..."]) --> G0
    G0["G0: Parser signature guardrail\n(check-errorsink-option-signatures.sh)"]
    G0 -->|pass| G1["G1: Rust workspace compile\n(cargo check --workspace)"]
    G1 -->|pass| G2["G2: Spec tools compile\n(cd spec/tools && cargo check)"]
    G2 -->|pass| G3["G3: Spec runtime tools compile\n(spec/runtime-tools)"]
    G3 -->|pass| G4["G4: CHAT manual anchor links\n(check-chat-manual-anchors.sh)"]
    G4 -->|pass| G5["G5: Generated parser corpus\nequivalence suite"]
    G5 -->|pass| G6["G6: Golden fragment validity\n(words + tiers)"]
    G6 -->|pass| G7["G7: Bare-timestamp\nregression gate"]
    G7 -->|pass| G8["G8: Reference corpus\nsemantic equivalence\n(97 files)"]
    G8 -->|pass| G9["G9: %wor tier parsing\nand alignment"]
    G9 -->|pass| G10["G10: Golden tier roundtrip\n(%mor, %gra, %pho, %wor)"]
    G10 -->|pass| G11["G11: Reference corpus\nnode coverage audit"]
    G11 -->|pass| G12["G12: Generated artifacts\nmatch committed sources"]
    G12 -->|pass| G13["G13: Fuzz workspace\nisolation"]
    G13 -->|pass| G14["G14: Imported Batchalign\nRust/PyO3 gate"]
    G14 -->|pass| done(["All gates passed"])
    G0 & G1 & G2 & G3 & G4 & G5 & G6 & G7 & G8 & G9 & G10 & G11 & G12 & G13 & G14 -->|fail| abort(["Pipeline stops"])
```

| Gate | Check |
|------|-------|
| G0 | Parser signature guardrail |
| G1 | Rust workspace compile check |
| G2 | Spec tools compile check |
| G3 | Spec runtime tools compile check |
| G4 | CHAT manual anchor links |
| G5 | Generated parser corpus equivalence suite |
| G6 | Golden fragment validity (words + tiers) |
| G7 | Bare-timestamp regression gate |
| G8 | Reference corpus semantic equivalence (97 files) |
| G9 | %wor tier parsing and alignment |
| G10 | Golden tier roundtrip (%mor, %gra, %pho, %wor) |
| G11 | Reference corpus node coverage |
| G12 | Generated artifacts match committed sources |
| G13 | Fuzz workspace isolation |
| G14 | Imported Batchalign Rust/PyO3 gate |

## Running Specific Tests

```bash
# Single test by name
cargo nextest run test_name

# Tests in a specific crate
cargo nextest run -p talkbank-model

# Tests matching a pattern
cargo nextest run -- mor

# With output
cargo nextest run --no-capture
```

## What to Run When

| What you changed | Run |
|-----------------|-----|
| Grammar (`grammar.js`) | `cd grammar && tree-sitter generate && tree-sitter test` then `bazel test //crates/core/talkbank-parser-tests/...` |
| Parser (CST-to-model) | `cargo nextest run -p talkbank-parser` |
| Model (types, validation, alignment) | `cargo nextest run -p talkbank-model` |
| CLAN command | `cargo nextest run -p clan-core -E 'test(command_name)'` + golden test |
| CLI (chatter args, dispatch) | `cargo nextest run -p chatter-cli` |
| LSP | `cargo nextest run -p chatter-lsp` |
| Spec files | `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && bazel build //... && bazel test //...` |
| Pre-merge (any change) | `bazel build //... && bazel test //...` |
| Pre-push (quick) | `bazel build //... && bazel test //...` |

## Mutation Testing

Use `cargo-mutants` to find code that can be changed without any test failing — the true coverage gaps.

```bash
# Install (once)
cargo install cargo-mutants

# Run against a specific crate (--jobs 1 to avoid OOM on 64 GB machines)
cargo mutants -p talkbank-parser --timeout 120 --jobs 1

# Run against CLAN commands
cargo mutants -p clan-core --timeout 120 --jobs 1

# Review results
cat mutants.out/missed.txt    # Mutations no test caught
cat mutants.out/caught.txt    # Mutations properly detected
```

Mutation testing is not part of CI but should be run periodically (after major changes) to find untested logic paths. Results guide where to add new tests.

Configuration: `mutants.toml` at the repo root excludes trivial functions.

## Adding Tests

- **Model tests**: add to the relevant crate's `tests/` directory or `#[cfg(test)]` module
- **Parser tests**: if the change is about grammar shape or validation contracts,
  add or update specs and regenerate with `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`
- **Error corpus tests**: add a `.cha` file to `tests/error_corpus/` and update `expectations.json`
- **CLAN command tests**: add golden test cases in `tests/clan_golden/` using the manifest-driven `ParityCase` / `RustSnapshotCase` pattern
