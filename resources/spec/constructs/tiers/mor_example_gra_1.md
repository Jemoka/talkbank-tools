# mor_example_gra_1

UD-style %mor tier used with %gra: single proper noun (INCROOT pattern).

## Input

```mor_dependent_tier
%mor:	PROPN|Mommy .
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
