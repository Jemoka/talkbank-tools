# com_example_1

Example com tier

## Input

```com_dependent_tier
%com:	"foo" isn't just <5> now
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
