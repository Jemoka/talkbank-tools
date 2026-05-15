# language_suffix

Word with language suffix — `@s:code` or `@s:code1+code2`.

## Input

```main_tier
*CHI:	hao3@s:eng+zho .
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
                (word_segment))
              (word_lang_suffix
                (word_lang_codes
                  (language_code)
                  (language_code))))))))
    (utterance_end
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
