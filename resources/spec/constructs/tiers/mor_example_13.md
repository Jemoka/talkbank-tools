# mor_example_13

UD-style %mor tier: five-word sentence with mixed plain and featured words.

## Input

```mor_dependent_tier
%mor:	DET|the NOUN|people AUX|be-Pres VERB|make-Prog NOUN|cake-Plur .
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
        (mor_lemma)))
    (whitespaces)
    (mor_content
      (mor_word
        (mor_pos)
        (pipe)
        (mor_lemma)
        (mor_feature
          (hyphen)
          (mor_feature_value))))
    (whitespaces)
    (mor_content
      (mor_word
        (mor_pos)
        (pipe)
        (mor_lemma)
        (mor_feature
          (hyphen)
          (mor_feature_value))))
    (whitespaces)
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
