# participants_single

@Participants header with single participant

## Input

```participants_header
@Participants:	CHI Target_Child
```

## Expected CST

```cst
(participants_header
  (participants_prefix)
  (header_sep
    (colon)
    (tab)
  )
  (participants_contents
    (participant
      (speaker)
      (whitespaces
        (whitespace
          (space)
        )
      )
      (participant_word)
    )
  )
  (newline)
)
```

## Metadata

- **Level**: header
- **Category**: header
