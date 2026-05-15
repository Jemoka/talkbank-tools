# Grammar System and Token Governance

**Status:** Current
**Last updated:** 2026-03-24 00:01 EDT

## Current Reality
`grammar/grammar.js` encodes substantial implicit language knowledge directly in regex exclusions,
reserved symbol lists, and leniency decisions. Example areas:
- word segment forbidden start/rest classes,
- CA delimiter/element symbol groups,
- event segment exclusions,
- hand-maintained coupling between comments and token rules.

This is currently powerful but fragile.

## Primary Failure Modes
1. New symbolic token added in one place but not in exclusion sets.
2. Parser behavior changes silently due to regex class edits.
3. Generated node types drift from assumptions in spec tooling.
4. Lenient parsing choices become undocumented policy.

## Canonical Target Design
Implement a generated symbol registry as the single source of token constraints.

### Proposed Registry Artifacts
- `resources/spec/symbols/symbol_registry.yaml` (human-authored intent):
  - symbol string
  - category (delimiter, continuation, overlap, punctuation, etc.)
  - contexts where reserved/allowed
  - parse role and precedence notes
- Generated outputs:
  - `grammar/src/generated_symbol_sets.js`
  - `crates/talkbank-model/src/generated/symbol_sets.rs`
  - `crates/spec/talkbank-spec-testgen/src/generated/symbol_sets.rs`
  - docs: [Symbol Registry](symbol-registry.md)

## Grammar Refactor Requirements
1. Replace large manual regex strings with generated character classes.
2. Keep final grammar readable by preserving semantic names in generated constants.
3. Distinguish clearly between:
  - syntax permissiveness,
  - semantic validation restrictions.
4. Add comments only for design rationale, not for duplicating manual references.

## Node Type Drift Controls
- Enforce regeneration and consistency checks:
  - grammar source change must regenerate parser and node types,
  - node type constants consumed by `spec/tools` and parser code must compile,
  - CI fails if generated files differ from committed state.

## Leniency Policy
Explicitly classify every lenient parse behavior:
- Parse-lenient + validate-strict.
- Parse-lenient + validate-warning.
- Parse-strict (hard fail).

Document this matrix in the [Leniency Policy](leniency-policy.md).

## Grammar Test Strategy
1. Keep corpus tests generated from `spec/constructs`.
2. Add targeted hand-authored edge tests for symbol boundary interactions.
3. Add mutation-style tests for forbidden-character regressions.
4. Add parser equivalence tests for tokenizer-sensitive cases.

## Acceptance Criteria
- No manual reserved-symbol duplication in `grammar.js`.
- Symbol registry is generated to all required consumers.
- Grammar modifications cannot land with stale generated artifacts.
- Every special token category has explicit policy documentation.
