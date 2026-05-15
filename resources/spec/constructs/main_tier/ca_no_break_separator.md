# ca_no_break_separator

CA no-break `≈` remains a separator at the end of a main tier rather than an
utterance terminator. Based on `resources/corpus/reference/ca/intonation.cha`.

## Input

```main_tier
*SPK:	no break continuation ⇘ ≈
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
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word))))
      (whitespaces)
      (separator
        (non_colon_separator
          (falling_to_low)))
      (whitespaces)
      (separator
        (non_colon_separator
          (ca_no_break))))
    (utterance_end
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
