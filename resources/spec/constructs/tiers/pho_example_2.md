# pho_example_2

Example %pho tier: a b

## Input

```pho_dependent_tier
%pho:	a b
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
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
