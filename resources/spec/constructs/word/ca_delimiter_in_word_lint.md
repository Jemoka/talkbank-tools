# ca\_delimiter\_in\_word\_lint

Word wrapped in CA delimiter markers (voice quality) — verifies `ca_delimiter`
at word boundaries parses correctly despite prec non-propagation.

Regression gate for grammar lint finding: prec(6) on `standalone_word` does not
propagate to `ca_delimiter` children. Harmless because `ca_delimiter` uses
`token(prec(10, ...))` for DFA-level disambiguation.

## Input

```standalone_word
°soft°
```

## Expected CST

```cst
(standalone_word
  (word_body
    (ca_delimiter)
    (word_segment)
    (ca_delimiter)))
```

## Metadata

- **Level**: word
- **Category**: word
