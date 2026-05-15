# com_example_2

Example com tier

## Input

```com_dependent_tier
%com:	TTHIS can (happen) in "quotes" or even with !! sometimes.
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
