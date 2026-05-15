# form_marker

Word with form type marker — `@letter` suffix indicating word form.

## Input

```main_tier
*CHI:	doggie@c .
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
              (form_marker))))))
    (utterance_end
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
