# overlap\_standalone

Standalone overlap markers (space-separated from words) parse as
`overlap_point` content items, not as part of a word. Verifies the
GLR conflict `[$.contents, $.word_body]` resolves correctly when
overlap markers are whitespace-delimited.

Regression gate for degenerate `contents` lint finding: `contents`
can match bare `overlap_point` as sole content — this is correct for
CA transcription.

## Input

```main_tier
*CHI:	⌈ hello ⌉ .
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
      (overlap_point)
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (overlap_point)
      (whitespaces))
    ending: (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
