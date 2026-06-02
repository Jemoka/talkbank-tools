# Symbol Registry

This directory contains shared symbol definitions used to keep CHAT token policy synchronized
across grammar and tooling.

## Source of Truth
- `symbol_registry.json` is the canonical source.
- The registry currently defines:
  - CA delimiter and element symbol sets.
  - Word-segment forbidden character classes (start, rest, common).
  - Event-segment forbidden character classes (base, common).

## Validation
Before regeneration, validate the registry:

```bash
node resources/spec/symbols/validate_symbol_registry.js
```

Validation enforces:
- required category keys,
- single-scalar Unicode symbols,
- no duplicates,
- lexicographic ordering in each category,
- disjoint CA delimiter and element sets.

## Generated Output
- `grammar/src/generated_symbol_sets.js` (consumed by `grammar/grammar.js`)
- `crates/core/talkbank-model/src/generated/symbol_sets.rs`
- `crates/spec/talkbank-spec-testgen/src/generated/symbol_sets.rs`

## Regeneration
```bash
node resources/spec/symbols/validate_symbol_registry.js && node scripts/generate-symbol-sets.js && node resources/spec/symbols/generate_rust_symbol_sets.js
```

Do not edit generated files manually.
