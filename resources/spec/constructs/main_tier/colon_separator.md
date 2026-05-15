# colon\_separator

Standalone colon between words must parse as `separator(colon)`, not as
`lengthening` inside a word. The DFA produces `lengthening` (prec 5) for
`:` but the parser rejects it as word_body start — it falls through to
`separator(colon)`.

Regression gate for grammar lint finding: `lengthening` shadows `colon`
at the DFA level. Harmless because parser-level rules filter.

## Input

```main_tier
*CHI:	hello : world .
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
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (separator
        (colon))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces))
    ending: (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
