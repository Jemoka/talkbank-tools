# pho_example_10

Example %pho tier: foo+bar

## Input

```pho_dependent_tier
%pho:	foo+bar
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
        (plus)
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
