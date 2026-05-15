# group_multiple_annotations

Angle bracket group with multiple space-separated scoped annotations.
All annotations must be collected as part of the same group, not split
into separate content items.

Found in: childes-eng-na, aphasia-data, asd-data (6,900+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```main_tier
*CHI:	<really> [?] [<] .
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
          (group_with_annotations
            (left_angle_bracket)
            (contents
              (content_item
                (base_content_item
                  (word_with_optional_annotations
                    (standalone_word
                      (word_body
                        (initial_word_segment)))))))
            (right_angle_bracket)
            (base_annotations
              (uncertain)
              (overlap_precedes))))))
    (utterance_end ...)))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
