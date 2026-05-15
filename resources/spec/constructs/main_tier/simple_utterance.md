# simple_utterance

Simple main tier with single word

## Input

```main_tier
*CHI:	hello .
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
    )
    (utterance_end
      (whitespaces
        (whitespace
          (space)
        )
      )
      (period)
      (newline)
    )
  )
)
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
