# ca\_element\_in\_word\_lint

Word with internal CA pitch marker — verifies `ca_element` inside `word_body`
parses correctly despite prec non-propagation.

Regression gate for grammar lint finding: prec(6) on `standalone_word` does not
propagate to `ca_element` children. Harmless because `ca_element` uses
`token(prec(10, ...))` for DFA-level disambiguation.

## Input

```standalone_word
he↑llo
```

## Expected CST

```cst
(standalone_word
  (word_body
    (word_segment)
    (ca_element)
    (word_segment)))
```

## Metadata

- **Level**: word
- **Category**: word
