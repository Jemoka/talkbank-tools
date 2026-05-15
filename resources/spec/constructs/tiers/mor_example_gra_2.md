# mor_example_gra_2

UD-style %mor tier used with %gra: question with post-clitic (where~is your cup?).

## Input

```mor_dependent_tier
%mor:	ADV|where~AUX|be-Pres-S3 DET|your NOUN|cup ?
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
        (mor_lemma))
      (mor_post_clitic
        (tilde)
        (mor_word
          (mor_pos)
          (pipe)
          (mor_lemma)
          (mor_feature
            (hyphen)
            (mor_feature_value))
          (mor_feature
            (hyphen)
            (mor_feature_value)))))
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
    (question))
  (newline))
```

## Metadata

- **Level**: tier
- **Category**: tiers
