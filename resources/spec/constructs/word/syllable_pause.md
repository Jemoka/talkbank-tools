# syllable_pause

Syllable pause marker `^` between syllables within a word: `ja^ja^ja@o`.

## Input

```main_tier
*CHI:	ja^ja^ja@o .
```

## Expected CST

```cst
(main_tier
  (star)
  (speaker)
  (colon)
  (tab)
  (tier_body
    (contents
      (content_item
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment)
                (syllable_pause)
                (word_segment)
                (syllable_pause)
                (word_segment))
              (form_marker))))))
    (utterance_end
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
