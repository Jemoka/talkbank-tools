# event_with_annotation

Event with scoped annotations. Events (`&=action`) can carry content
annotations like `[*]`, `[=! text]`, `[>]`, but retrace markers (`[/]`,
`[//]`) are silently dropped (retraces are not applicable to events).

grammar.js: events are wrapped in `nonword_with_optional_annotations`.
TreeSitterParser drops retraces via `parsed.content` (ignoring `.retrace`).

Found in: aphasia-data, childes-eng-na (3,472+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```main_tier
*CHI:	one &=ges [//] the computer do it .
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
                (initial_word_segment))))))
      (whitespaces ...)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment)))
            (whitespaces ...)
            (base_annotations
              (retrace_complete)))))
      (whitespaces ...)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (initial_word_segment)))))))
    (utterance_end ...)))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
