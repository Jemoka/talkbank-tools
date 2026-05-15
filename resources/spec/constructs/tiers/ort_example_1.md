# ort_example_1

Orthography tier as plain dependent text.

## Input

```utterance
*CHI:	hello .
%ort:	hello
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (ort_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
