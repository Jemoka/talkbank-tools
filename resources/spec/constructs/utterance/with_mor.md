# with_mor

Utterance with main tier and %mor dependent tier

## Input

```utterance
*CHI:	hello .
%mor:	co|hello .
```

## Expected CST

```cst
(utterance
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
  (mor_dependent_tier
    (mor_tier_prefix)
    (tier_sep
      (colon)
      (tab)
    )
    (mor_contents
      (mor_content
        (mor_compound_word
          (mor_word
            (mpos
              (mor_category
                (mor_word_segment)
              )
            )
            (pipe)
            (stem
              (mor_word_segment)
            )
          )
        )
      )
      (whitespaces
        (whitespace
          (space)
        )
      )
      (period)
    )
    (newline)
  )
)
```

## Metadata

- **Level**: utterance
- **Category**: utterance
