# overlap_enclosed

Word enclosed in overlap markers

## Input

```standalone_word
⌈is⌉
```

## Expected CST

```cst
(standalone_word
  (word_body
    (word_content_nontext
      (overlap_point))
    (word_content
      (word_segment))
    (word_content
      (overlap_point))
  )
)
```

## Metadata

- **Level**: word
- **Category**: overlap
