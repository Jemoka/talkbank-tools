# gra_incroot

Example %gra tier with INCROOT pattern

## Input

```gra_dependent_tier
%gra:	1|0|INCROOT 2|1|PUNCT
```

## Expected CST

```cst
(gra_dependent_tier
  (gra_tier_prefix)
  (tier_sep
    (colon)
    (tab)
  )
  (gra_contents
    (gra_relation
      (gra_index)
      (pipe)
      (gra_head)
      (pipe)
      (gra_relation_name)
    )
    (whitespaces
      (whitespace
        (space)
      )
    )
    (gra_relation
      (gra_index)
      (pipe)
      (gra_head)
      (pipe)
      (gra_relation_name)
    )
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
