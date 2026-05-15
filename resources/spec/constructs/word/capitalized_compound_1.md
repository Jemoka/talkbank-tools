# capitalized\_compound\_1

Compound word with capitalized parts

## Input

``` standalone_word
Foo+Bar
```

## Expected CST

``` cst
(standalone_word
  (word_body
    (initial_word_segment)
    (word_content
      (plus)
    )
    (word_content
      (word_segment)
    )
  )
)
```

## Metadata

- **Level**: word
- **Category**: compounds
