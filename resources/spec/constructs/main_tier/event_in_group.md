# event_in_group

Event (`&=action`) inside an angle bracket group `<...>` with overlap annotation.
The closing `>` must NOT be part of the event description text.

Found in: aphasia-data, childes-eng-na-data, asd-data (570+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```main_tier
*CHI:	<&=laughs> [<] .
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
                  (event
                    (event_marker)
                    (event_segment)))))
            (right_angle_bracket)
            (base_annotations
              (overlap_precedes)))))))
  (utterance_end ...))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
