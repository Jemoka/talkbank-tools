# pianissimo

Word wrapped in double CA delimiter `°°` (pianissimo — very soft voice).
This is a stacked marker: two identical `ca_delimiter` tokens before the
text, two after. Tests that repeated delimiters parse without ERROR nodes.

## Input

```standalone_word
°°hello°°
```

## Expected CST

```cst
(standalone_word
  (word_body
    (ca_delimiter)
    (ca_delimiter)
    (word_segment)
    (ca_delimiter)
    (ca_delimiter)))
```

## Metadata

- **Level**: word
- **Category**: word
