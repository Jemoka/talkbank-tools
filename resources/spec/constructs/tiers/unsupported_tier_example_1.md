# unsupported_tier_example_1

Unknown dependent tier label captured by the fallback rule.

## Input

```utterance
*CHI:	okay .
%zzz:	custom notes
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (unsupported_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
