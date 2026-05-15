# act_example_1

Action tier attached to an utterance.

## Input

```utterance
*CHI:	look .
%act:	points to toy
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (act_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
