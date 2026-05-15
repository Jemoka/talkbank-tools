# event_compound

Compound event with colon separator (`&=clears:throat`).
The colon is part of the event segment text, not a structural delimiter.

## Input

```main_tier
*CHI:	&=clears:throat .
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
          (event
            (event_marker)
            (event_segment)))))
  (utterance_end ...))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
