# alt_example_1

Alternative tier content for an utterance.

## Input

```utterance
*CHI:	I goed there .
%alt:	I went there
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (alt_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
