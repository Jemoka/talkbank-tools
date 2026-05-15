# overlap\_digit\_1

Overlap marker with digit 1 (⌊1) must parse as a single `overlap_point`
token, not as `overlap_point(⌊)` + `word_segment(1)`. The digit is part
of the overlap marker notation.

At the grammar level, [1-9] is accepted. The validator (E373) rejects
index 1 — valid CHAT range is 2-9. This spec verifies the grammar
doesn't silently split the digit from the marker.

Regression gate for overlap_point regex change from `[2-9]?` to `[1-9]?`.

## Input

```standalone_word
⌊1hello
```

## Expected CST

```cst
(standalone_word
  (word_body
    (overlap_point)
    (word_segment)))
```

## Metadata

- **Level**: word
- **Category**: word
