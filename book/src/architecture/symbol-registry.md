# Symbol Registry Architecture

**Status:** Current
**Last updated:** 2026-03-24 00:01 EDT

## Purpose
`resources/spec/symbols/symbol_registry.json` is the canonical source of token/symbol classes used by
CHAT grammar tokenization policy.

## Scope
The registry currently governs:
- CA delimiter symbols,
- CA element symbols,
- word segment forbidden symbol classes,
- event segment forbidden symbol classes.

## Governance Rules
1. Symbol changes must be made only in `resources/spec/symbols/symbol_registry.json`.
2. Registry must pass validation:
   - `node resources/spec/symbols/validate_symbol_registry.js`
3. Grammar symbol sets must be regenerated after any registry change:
   - `node scripts/generate-symbol-sets.js`
4. Generated files are read-only and must not be edited manually.

## Determinism Requirements
- Every category list in the registry must be lexicographically sorted.
- Duplicate symbols are forbidden.
- `ca_delimiter_symbols` and `ca_element_symbols` must be disjoint.

These constraints keep generated outputs stable and review diffs minimal.

## Consuming Outputs
Generated symbol constants are emitted to:
- `grammar/src/generated_symbol_sets.js`

`grammar/grammar.js` imports from this generated module to avoid manual duplication of
critical symbol policy.

## Change Workflow
1. Edit registry JSON.
2. Run registry validation.
3. Regenerate symbol sets.
4. Run grammar generation/tests.
5. Run parser equivalence tests.
6. Commit source + generated outputs together.

## Auditability
Registry and generated outputs are covered by `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code` and CI checks, so drift
between source policy and consumed grammar constants is merge-blocking.
