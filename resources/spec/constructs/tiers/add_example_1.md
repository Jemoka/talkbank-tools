# add_example_1

Addressee tier as dependent free text.

## Input

```utterance
*CHI:	look here .
%add:	MOT
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (add_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
