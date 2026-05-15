# pho_example_6

Example %pho tier: ‹a b›  c ‹d e›  f g

## Input

```pho_dependent_tier
%pho:	‹a b›  c ‹d e›  f g ‹h i›  j
```

## Expected CST

```cst
(pho_dependent_tier
  (pho_tier_prefix)
  (tier_sep
    (colon)
    (tab)
  )
  (pho_groups
    (pho_group
      (pho_begin_group)
      (pho_grouped_content
        (pho_words
          (pho_word)
        )
        (whitespaces
          (whitespace
            (space)
          )
        )
        (pho_words
          (pho_word)
        )
      )
      (pho_end_group)
    )
    (whitespaces
      (whitespace
        (space)
      )
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_words
        (pho_word)
      )
    )
    (whitespaces
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_begin_group)
      (pho_grouped_content
        (pho_words
          (pho_word)
        )
        (whitespaces
          (whitespace
            (space)
          )
        )
        (pho_words
          (pho_word)
        )
      )
      (pho_end_group)
    )
    (whitespaces
      (whitespace
        (space)
      )
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_words
        (pho_word)
      )
    )
    (whitespaces
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_words
        (pho_word)
      )
    )
    (whitespaces
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_begin_group)
      (pho_grouped_content
        (pho_words
          (pho_word)
        )
        (whitespaces
          (whitespace
            (space)
          )
        )
        (pho_words
          (pho_word)
        )
      )
      (pho_end_group)
    )
    (whitespaces
      (whitespace
        (space)
      )
      (whitespace
        (space)
      )
    )
    (pho_group
      (pho_words
        (pho_word)
      )
    )
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
