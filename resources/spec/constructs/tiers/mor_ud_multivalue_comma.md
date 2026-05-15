# mor_ud_multivalue_comma

UD multi-value features: commas separate multiple values within a single feature
(e.g., `Int,Rel` is one feature value, not two separate features). This follows
Universal Dependencies conventions.

## Input

```mor_dependent_tier
%mor:	PRON|what-Acc-Int,Rel .
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
          (mor_feature_value))
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
