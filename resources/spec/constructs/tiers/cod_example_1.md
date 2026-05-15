# cod_example_1

Coding tier as dependent annotation text.

## Input

```utterance
*CHI:	hello .
%cod:	codeA
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (cod_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
