# com_example_3

Example com tier

## Input

```com_dependent_tier
%com:	a-a-and - -- we get *** /this\ or #{this} with @^$ +  and the rest.
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
