# overlap_with_action

Action marker 0 within overlap markers should parse as separate elements

## Input

```main_tier
*CHI:	⌈ 0 &=laughter⌉ .
```

## Expected CST

```cst
(main_tier
  (star)
  speaker: (speaker)
  (colon)
  (tab)
  (tier_body
    content: (contents
      (content_item
        (overlap_point))
      (whitespaces (whitespace (space)))
      (content_item
        (base_content_item
          (action_with_optional_annotations
            marker: (zero))))
      (whitespaces (whitespace (space)))
      (content_item
        (base_content_item
          (event_with_optional_annotations
            event: (event
              (event_marker)
              description: (event_segment)))))
      (overlap_point)
      (whitespaces (whitespace (space))))
    ending: (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: overlap_points
