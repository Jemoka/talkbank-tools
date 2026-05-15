# Spec System

**Status:** Current
**Last updated:** 2026-03-24 01:32 EDT

Specifications in `spec/` are the authoritative source of truth for the CHAT format. They drive grammar artifact generation, validation/error docs, and targeted test generation.

**Historical note:** This system was originally shaped during a
dual-parser era. The chumsky-based direct parser was removed in
March 2026. Today the canonical parser is tree-sitter
(`talkbank-parser`); a second implementation,
`talkbank-parser-re2c`, exists as a specification oracle and
high-throughput batch parser. Fragment specs remain valuable, but
synthetic tree-sitter wrapper behavior is audit-only legacy unless a
page or test explicitly says otherwise.

## Spec Types

### Construct Specs (`resources/spec/constructs/`)

Each construct spec defines a valid CHAT pattern with its expected parse tree:

```markdown
# example_name

Description of what this example tests.

## Input

\```mor_dependent_tier
%mor:	VERB|eat .
\```

## Expected CST

\```cst
(mor_dependent_tier
  (mor_tier_prefix)
  ...)
\```

## Metadata

- **Level**: tier
- **Category**: tiers
```

The `Input` code fence label (e.g., `mor_dependent_tier`, `utterance`) selects
which template wraps the fragment into a full CHAT file for parsing.

That is an explicit **grammar/test templating** mechanism. It is useful, but it
does **not** by itself define honest isolated-fragment semantics for the direct
parser.

### Error Specs (`resources/spec/errors/`)

Each error spec defines an invalid CHAT pattern with expected error codes:

```markdown
# Error E301

## Metadata

- Code: E301
- Name: missing_participants
- Severity: Error
- Layer: parser

## Examples

### missing_participants_1

\```chat
@UTF8
@Begin
*CHI: hello .
@End
\```
```

Key metadata fields:
- **Layer: parser** — error caught during parsing (returns `Err`)
- **Layer: validation** — error caught after successful parse
- **Status: not_implemented** — generates `#[ignore]` tests

### Symbol Registry (`resources/spec/symbols/`)

`symbol_registry.json` defines character sets used by both the grammar and Rust crates. Running `node resources/spec/symbols/validate_symbol_registry.js && node scripts/generate-symbol-sets.js && node resources/spec/symbols/generate_rust_symbol_sets.js` generates:
- JavaScript constants for `grammar.js`
- Rust constants for model validation

## Test Generation

Running `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` currently executes three generators:

### 1. Tree-sitter Corpus Tests

`gen_tree_sitter_tests` reads construct specs and error specs, then:
- Wraps each `Input` in a template to create a full CHAT file
- Parses with tree-sitter and checks for error nodes
- Writes `Expected CST` to `grammar/test/corpus/`

For error specs, it captures the actual parse (with ERROR nodes) as the expected tree.

### 2. Rust Tests

`gen_rust_tests` generates Rust test functions:
- Construct specs become parse-and-compare tests
- Parser-layer error specs become `parser.parse_chat_file()` tests expecting `Err`
- Validation-layer error specs become parse-then-validate tests

Output: `crates/talkbank-parser-tests/tests/generated/`

The generated suites are useful as grammar/audit support and regression
coverage, but they are not the sole authority for parser semantics.

### 3. Error Documentation

`gen_error_docs` generates markdown pages for each error code at `book/src/operations/errors/`.

## Workflow After Spec Changes

```bash
cd talkbank-tools
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs     # Regenerate the affected spec-driven artifacts
bazel build //... && bazel test //...       # Run pre-merge verification gates
```

Never hand-edit generated artifacts — always regenerate from specs.

## Post-Bootstrap Doctrine

- `spec/tools` remains the generator/validator for grammar corpus tests, error
  docs, and shared symbol artifacts.
- `talkbank-parser-tests` owns parser equivalence and roundtrip contracts.
- Isolated grammar additions should usually need two things: one grammar
  corpus example and one full-file fixture. They should not require
  the old bootstrap ritual unless generated
  artifacts really changed.
