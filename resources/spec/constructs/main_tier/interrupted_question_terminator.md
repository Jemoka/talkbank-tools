# interrupted_question_terminator

Main tier ending with interrupted-question terminator `+/?`.

## Input

```main_tier
*CHI:	hello +/?
```

## Expected CST

```cst
(main_tier
  (tier_body
    (utterance_end
      (interrupted_question)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
