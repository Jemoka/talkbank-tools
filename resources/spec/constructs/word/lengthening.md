# lengthening

Word-internal lengthening — trailing colons indicate prolonged sound.

## Input

```main_tier
*CHI:	no::: .
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
                (word_segment)
                (lengthening)))))))
    (utterance_end
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
