# com_example_5

Example com tier

## Input

```com_dependent_tier
%com:	and there are the CA characters ↓↑∆∇t☺ and so on (see manual)
```

## Expected CST

```cst
(com_dependent_tier
  (com_tier_prefix)
  (tier_sep
    (colon)
    (tab)
  )
  (text_with_bullets_and_pics
    (text_segment)
  )
  (newline)
)
```

## Metadata

- **Level**: tier
- **Category**: tiers
