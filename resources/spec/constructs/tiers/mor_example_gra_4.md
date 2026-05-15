# mor_example_gra_4

UD-style %mor tier used with %gra: complex sentence with multiple post-clitics and question terminator.

## Input

```mor_dependent_tier
%mor:	PRON|you VERB|go-Prog~PART|to VERB|put DET|the NOUN|choo_choo~AUX|be-Pres-S3 NOUN|wheel ADV|on ?
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
          (mor_feature_value)))
      (mor_post_clitic
        (tilde)
        (mor_word
          (mor_pos)
          (pipe)
          (mor_lemma))))
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
