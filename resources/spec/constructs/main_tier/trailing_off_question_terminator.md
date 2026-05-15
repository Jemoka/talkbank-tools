# trailing_off_question_terminator

Main tier ending with trailing-off question terminator `+..?`.

## Input

```main_tier
*CHI:	hello +..?
```

## Expected CST

```cst
(main_tier
  (tier_body
    (utterance_end
      (trailing_off_question)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
