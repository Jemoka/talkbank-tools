# mor_example_4

UD-style %mor tier: locative adverb, copula with features, determiner, and noun.

## Input

```mor_dependent_tier
%mor:	ADV|here AUX|be-Pres-S3 DET|a NOUN|dog .
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
