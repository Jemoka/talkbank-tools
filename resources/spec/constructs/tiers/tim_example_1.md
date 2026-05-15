# tim_example_1

Time-alignment dependent tier with strict timestamp range.

## Input

```utterance
*CHI:	hello .
%tim:	17:30-18:00
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (tim_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
