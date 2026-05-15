# mor_example_10

UD-style %mor tier: Key=Value feature format (Number=Plur).

## Input

```mor_dependent_tier
%mor:	NOUN|cookie-Number=Plur .
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
        (mor_lemma)
        (mor_feature
          (hyphen)
          (mor_feature_value))))
    (whitespaces)
    (period))
  (newline))
```

## Metadata

- **Level**: tier
- **Category**: tiers
