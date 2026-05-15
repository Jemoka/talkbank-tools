# mor_example_gra_3

UD-style %mor tier used with %gra: compound as single lemma (ice_cream).

## Input

```mor_dependent_tier
%mor:	NOUN|ice_cream .
```

## Expected CST

```cst
(mor_dependent_tier
  (mor_tier_prefix)
  (tier_sep
    (colon)
    (tab))
  (mor_contents
    (mor_content
      (mor_word
        (mor_pos)
        (pipe)
        (mor_lemma)))
    (whitespaces)
    (period))
  (newline))
```

## Metadata

- **Level**: tier
- **Category**: tiers
