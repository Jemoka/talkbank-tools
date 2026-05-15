# def_example_1

Definitions tier as dependent text.

## Input

```utterance
*CHI:	hello .
%def:	hello=salutation
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (def_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
