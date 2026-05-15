# sin\_zero\_marker

Zero marker in %sin tier — verifies `0` parses as `sin_word > zero` (not ERROR).

Regression gate for grammar lint finding: `zero` (prec 3) shadows `sin_word`
regex at the DFA level. The shadow is harmless because `sin_word` explicitly
lists `$.zero` as an alternative.

## Input

```utterance
*CHI:	hello .
%sin:	0
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (sin_dependent_tier
    (sin_tier_prefix)
    (tier_sep ...)
    (sin_group
      (sin_word
        (zero)))))
```

## Metadata

- **Level**: tier
- **Category**: tiers
