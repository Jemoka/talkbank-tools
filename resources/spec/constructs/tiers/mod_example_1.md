# mod_example_1

Model tier with phonology-style grouped content.

## Input

```utterance
*CHI:	hello .
%mod:	abc
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (mod_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
