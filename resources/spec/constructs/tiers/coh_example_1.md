# coh_example_1

Cohesion tier as dependent annotation text.

## Input

```utterance
*CHI:	hello .
%coh:	ref1->ref2
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (coh_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
