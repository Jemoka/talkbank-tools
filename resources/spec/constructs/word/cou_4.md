# cou\_4

Word with nonword marker, lengthening

## Input

``` standalone_word
&~cou:
```

## Expected CST

``` cst
(standalone_word
  (word_prefix
    (nonword_marker)
  )
  (word_body
    (initial_word_segment)
    (word_content
      (colon)
    )
  )
)
```

## Metadata

- **Level**: word
- **Category**: scoped
