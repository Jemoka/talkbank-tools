# mor_example_6

UD-style %mor tier: pronoun, auxiliary with features, and adjective.

## Input

```mor_dependent_tier
%mor:	PRON|he AUX|be-Pres-S3 ADJ|noisy .
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
