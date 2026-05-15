# gra_complex

Example %gra tier with complex structure (SUBJ, ROOT, OBJ, PUNCT)

## Input

```gra_dependent_tier
%gra:	1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
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
