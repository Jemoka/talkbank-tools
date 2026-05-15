# lengthening\_between\_segments

Colon between word segments (no spaces) is prosodic lengthening, not a
separator. `a:b` is ONE word with lengthening after the first segment.

This is the core colon disambiguation case. The DFA produces `lengthening`
(prec 5) for `:`, and the parser accepts it inside `word_body` because
it follows a `word_segment`.

Regression gate for colon/lengthening shadow lint finding.

## Input

```standalone_word
a:b
```

## Expected CST

```cst
(standalone_word
  (word_body
    (word_segment)
    (lengthening)
    (word_segment)))
```

## Metadata

- **Level**: word
- **Category**: word
