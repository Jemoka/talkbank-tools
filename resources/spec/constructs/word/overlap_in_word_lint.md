# overlap\_in\_word\_lint

Word with internal overlap markers — verifies `overlap_point` inside `word_body`
parses correctly despite prec non-propagation from `standalone_word` (prec 6)
through `word_body` (prec 0).

Regression gate for grammar lint finding: prec(6) on `standalone_word` does not
propagate to `overlap_point` children. Harmless because `overlap_point` uses
`token(prec(10, ...))` for DFA-level disambiguation.

## Input

```standalone_word
butt⌈er⌉
```

## Expected CST

```cst
(standalone_word
  (word_body
    (word_segment)
    (overlap_point)
    (word_segment)
    (overlap_point)))
```

## Metadata

- **Level**: word
- **Category**: word
