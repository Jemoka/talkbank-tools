# mor_example_8

UD-style %mor tier: post-clitic with tilde (Mommy~is), two more words.

## Input

```mor_dependent_tier
%mor:	PROPN|Mommy~AUX|be-Pres-S3 DET|the NOUN|hero .
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
    (period))
  (newline))
```

## Metadata

- **Level**: tier
- **Category**: tiers
