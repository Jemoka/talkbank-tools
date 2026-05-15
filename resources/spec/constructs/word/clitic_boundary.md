# clitic_boundary

Clitic boundary `~` within a word: `le~ha` (Arabic clitic attachment).

## Input

```main_tier
*CHI:	le~ha .
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
                (tilde)
                (word_segment)))))))
    (utterance_end
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
