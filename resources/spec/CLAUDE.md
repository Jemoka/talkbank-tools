# spec — CHAT Specification

**Status:** Current
**Last updated:** 2026-06-01 00:52 PDT

## How This Works

Specs are the **single source of truth** for all CHAT grammar tests, parser
tests, and error documentation. You never hand-edit generated test files.

```
resources/spec/constructs/*.md  ─┐
                       ├──► just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs ──► grammar/test/corpus/*.txt  (tree-sitter tests)
resources/spec/errors/*.md      ─┤                 ──► crates/.../tests/generated/ (Rust tests)
                       │                 ──► book/src/operations/errors/*.md            (error docs)
crates/spec/talkbank-spec-testgen/templates/ ─┘
```

**`just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` wipes all three output directories and recreates them.**
If you hand-edit a file in `grammar/test/corpus/` or `tests/generated/`,
it will be deleted next time someone runs `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`.

## Spec Locations

| Location | Purpose |
|----------|---------|
| `resources/spec/constructs/` | Valid CHAT examples with expected CSTs |
| `resources/spec/errors/` | Invalid CHAT examples with expected error codes |
| → `grammar/test/corpus/` | Generated tree-sitter tests |
| → `crates/core/talkbank-parser-tests/tests/generated/` | Generated Rust parser/validation tests |
| → `book/src/operations/errors/` | Generated error documentation pages |

## Adding a Test

### 1. Create a spec file

Put it in the right directory under `resources/spec/constructs/` or `resources/spec/errors/`:

```
resources/spec/constructs/
├── header/      # @-header examples
├── main_tier/   # *SPK: line examples
├── tiers/       # %mor, %gra, %sin, %wor etc.
├── utterance/   # Full utterance (main + dependent tiers)
└── word/        # Word-internal structure
```

### 2. Spec format (constructs)

```markdown
# example_name

Description of what this tests.

## Input

```input_type
*CHI:	hello .
```

## Expected CST

```cst
(main_tier ...)
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
```

The `input_type` in the code fence (e.g., `main_tier`, `standalone_word`,
`utterance`) tells the generator which **template** to use for wrapping the
fragment in a full CHAT document. Templates live in `crates/spec/talkbank-spec-testgen/templates/`.

### 3. Spec format (errors)

```markdown
# E999 — Description

Error for some condition.

- **Code**: E999
- **Severity**: Error
- **Layer**: parser | validation
- **Status**: implemented | not_implemented

## Example

```chat
@UTF8
@Begin
...invalid content...
@End
```

## Expected Error Codes

- E999
```

### 4. Check templates

The `input_type` must match a `.tera` template in `crates/spec/talkbank-spec-testgen/templates/`.
If no template exists for your fragment type, create one. Templates wrap the
fragment in valid CHAT scaffolding so `tree-sitter test` can parse it.

Example (`crates/spec/talkbank-spec-testgen/templates/main_tier.tera`):
```
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
{{ input }}
@End
```

### 5. Regenerate and verify

```bash
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs          # Regenerate all outputs from specs
tree-sitter test       # Verify grammar tests pass (from grammar/)
bazel build //... && bazel test //...            # Full verification pipeline
```

## Key Commands

```bash
# Regenerate ALL generated artifacts from specs
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs

# Full CI-style check (grammar + symbols + tests + verification)
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code

# Verify spec format integrity
cargo run --bin validate_error_specs \
  --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml
```

## Generator Binaries (`crates/spec/talkbank-spec-testgen/src/bin/`)

| Binary | What it generates |
|--------|-------------------|
| `gen_tree_sitter_tests` | `grammar/test/corpus/*.txt` from constructs + errors |
| `gen_rust_tests` | `crates/.../tests/generated/*.rs` from constructs + errors |
| `gen_error_docs` | `book/src/operations/errors/*.md` from errors |
| `validate_spec` | Validates spec format integrity (no output) |
| `corpus_node_coverage` | Reports which grammar node types are exercised by `resources/corpus/reference/` |
| `extract_corpus_candidates` | Mines real `.cha` files for candidate specs (runtime-tools) |

## Cross-Spec Consistency

Error spec examples can be cross-referenced — the same `.cha` content may
appear in multiple specs with different expected error codes. When changing a
grammar rule so that previously-unparsable content now parses:

1. Update the primary error spec: change `Layer: parser` → `Layer: validation`
2. Audit `E316_auto.md`: remove examples that no longer produce E316
3. Run `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` to regenerate all artifacts
4. Run `bazel build //... && bazel test //...` to confirm

## See Also
- `crates/spec/talkbank-spec-testgen/CLAUDE.md` — generator implementation details
- `grammar/CLAUDE.md` — grammar change workflow
- `book/src/contributing/testing.md` — testing strategy
