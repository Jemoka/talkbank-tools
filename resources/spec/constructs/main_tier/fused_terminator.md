# fused_terminator

Fused word+terminator without space: `to+...` should parse as word `to` followed
by trailing_off terminator `+...`. CHAT requires a space before terminators, but
in practice this pattern appears in real-world files. The grammar should recover
by treating `+` as the start of a terminator rather than a compound marker when
the characters following `+` form a valid terminator.

See: https://talkbank.org/0info/manuals/CHAT.html#TrailingOff_Terminator

## Input

```main_tier
*CHI:	trying to+...
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
            (standalone_word)))))
    (utterance_end
      (trailing_off)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
