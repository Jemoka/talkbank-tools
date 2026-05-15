# pho_pause_as_word

In %pho and %mod tiers, `(..)` is a phonological word, NOT a pause.
The `pho_word` regex in grammar.js includes parentheses and dots in its
character class. Pauses are main-tier constructs only.

Found in: phon-eng-french-data/Eng-NA/Goad (227+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```utterance
*CHI:	hello there .
%pho:	wow (..) kijz .
```

## Expected CST

```cst
(utterance
  (main_tier ...)
  (pho_dependent_tier
    (pho_tier_prefix)
    (tier_sep)
    (pho_groups
      (pho_group
        (pho_words
          (pho_word)))
      (whitespaces ...)
      (pho_group
        (pho_words
          (pho_word)))
      (whitespaces ...)
      (pho_group
        (pho_words
          (pho_word)))
      (whitespaces ...)
      (pho_group
        (pho_words
          (pho_word))))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
