# flo_example_1

Flow tier as dependent annotation text.

## Input

```utterance
*CHI:	hello .
%flo:	smooth turn
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (flo_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
