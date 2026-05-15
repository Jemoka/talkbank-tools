# interruption_terminator

Main tier ending with interrupted terminator `+/.'`.

## Input

```main_tier
*CHI:	hello +/.
```

## Expected CST

```cst
(main_tier
  (tier_body
    (utterance_end
      (interruption)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
