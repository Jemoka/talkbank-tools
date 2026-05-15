# ca_arrow_end_separator

CA intonation arrows remain separators even when they appear at the end of a
main tier in CA mode. This pattern is covered by `resources/corpus/reference/ca/intonation.cha`.

## Input

```main_tier
*SPK:	level pitch →
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
            (standalone_word))))
      (separator
        (non_colon_separator
          (level_pitch))))
    (utterance_end
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
