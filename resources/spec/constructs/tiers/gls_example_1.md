# gls_example_1

Gloss tier as dependent annotation text.

## Input

```utterance
*CHI:	hello .
%gls:	HELLO
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (gls_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
