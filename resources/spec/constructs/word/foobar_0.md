# foobar\_0

Word with word-internal markers

## Input

``` standalone_word
foo^bar
```

## Expected CST

``` cst
(standalone_word
  (word_body
    (initial_word_segment)
    (word_content
      (caret)
    )
    (word_content
      (word_segment)
    )
  )
)
```

## Metadata

- **Level**: word
- **Category**: ca\_markers
