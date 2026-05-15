# shortening

Word-internal shortening — parenthesized text within a word.

## Input

```main_tier
*CHI:	(be)cause .
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
            (standalone_word
              (word_body
                (shortening
                  (word_segment))
                (word_segment)))))))
    (utterance_end
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
