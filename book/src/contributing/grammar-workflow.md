# Grammar Workflow

**Status:** Current
**Last updated:** 2026-03-23 23:49 EDT

The tree-sitter grammar at `grammar/grammar.js` is the formal definition of the CHAT format. Changes require careful validation.

The following diagram shows the complete regeneration pipeline. Every
step must pass before committing a grammar change.

```mermaid
flowchart TD
    edit(["Edit grammar/grammar.js"])
    generate["tree-sitter generate\n→ src/parser.c\n→ src/node-types.json"]
    grammar_test["tree-sitter test\n(160 corpus tests)"]
    rust_test["cargo test -p talkbank-parser\n(CST-to-model conversion)"]
    equiv["parser equivalence\n(74 reference corpus files)"]
    spec_check{"Grammar change\naffects spec examples?"}
    test_gen["just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs\n→ grammar/test/corpus/\n→ parser-tests/tests/generated/\n→ book/src/operations/errors/"]
    commit(["Commit"])

    edit --> generate --> grammar_test --> rust_test --> equiv --> spec_check
    spec_check -->|Yes| test_gen --> commit
    spec_check -->|No| commit
```

## Step-by-Step Procedure

### 1. Edit the Grammar

Modify `grammar.js` in the `grammar/` directory. Key design principles:

- Explicit whitespace (no `extras`)
- Precedence annotations to resolve ambiguities
- Named rules for all semantically meaningful nodes

### 2. Generate the Parser

```bash
cd grammar
tree-sitter generate
```

This produces `src/parser.c` and `src/node-types.json`. Never edit these files by hand.

### 3. Run Grammar Tests

```bash
tree-sitter test
```

All 160 tests must pass. Tests live in `test/corpus/` and are partially auto-generated from specs.

### 4. Run Parser Tests

```bash
cargo test -p talkbank-parser
```

This verifies the Rust parser wrapper handles all CST nodes correctly.

### 5. Run Parser Equivalence

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
```

Every file in the reference corpus must parse correctly. Each `.cha` file is its own test — nextest runs them in parallel and reports individual failures.

### 6. Regenerate Spec Tests

If the grammar change affects any spec examples:

```bash
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs
```

This regenerates tree-sitter corpus tests and other generated outputs that
still depend on the spec pipeline.

Do this when the grammar change actually affects generated artifacts.

### 7. Update node_types.rs

If new node types were added to the grammar, the generated `node_types.rs` in `talkbank-parser` needs updating. The spec tools handle this via `node-types.json`.

## Critical Policy

The reference corpus at `resources/corpus/reference/` (74 files) must pass parser equivalence at 100%. If a grammar change breaks even one file, revert immediately. The reference corpus is the ultimate arbiter of correctness.

## Common Patterns

### Adding a New Token

1. Define the token in `grammar.js`
2. Add handling in the Rust tier parser (match on the new node kind)
3. Add a spec construct example
4. Run the relevant generation and verification steps

For small, isolated syntax additions, the grammar workflow should stay local:

- one grammar change
- one grammar corpus example
- one full-file fixture if needed

### Changing a Rule

1. Modify the rule in `grammar.js`
2. `tree-sitter generate && tree-sitter test`
3. Update Rust parser if CST node structure changed
4. Update spec examples if the expected CST changed
5. Run full verification
