# self_interruption_terminator

Main tier ending with self-interruption terminator `+//.'`.

## Input

```main_tier
*CHI:	hello +//.
```

## Expected CST

```cst
(main_tier
  (tier_body
    (utterance_end
      (self_interruption)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
