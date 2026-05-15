# err_example_1

Error-coding tier as free text content.

## Input

```utterance
*CHI:	this one .
%err:	target lexical retrieval
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (err_dependent_tier ...))
```

## Metadata

- **Level**: tier
- **Category**: tiers
