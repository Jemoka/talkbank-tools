# sit_example_1

Situation tier attached to an utterance.

## Input

```utterance
*CHI:	I did it .
%sit:	child stacking blocks
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (sit_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
