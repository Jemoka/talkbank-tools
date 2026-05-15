# overlap_utterance

Utterance with a word containing overlap markers

## Input

```main_tier
*CHI:	who ⌈is⌉ ?
```

## Expected CST

```cst
(main_tier
  (star)
  (speaker)
  (colon)
  (tab)
  (tier_body
    (contents
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (initial_word_segment)
              )
            )
          )
        )
      )
      (whitespaces
        (whitespace
          (space)
        )
      )
      (content_item
        (base_content_item
          (word_with_optional_annotations
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
          )
        )
      )
    )
    (utterance_end
      (whitespaces
        (whitespace
          (space)
        )
      )
      (question)
      (newline)
    )
  )
)
```

## Metadata

- **Level**: main_tier
- **Category**: overlap
