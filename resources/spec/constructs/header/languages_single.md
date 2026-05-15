# languages_single

@Languages header with single language code

## Input

```languages_header
@Languages:	eng
```

## Expected CST

```cst
(languages_header
  (languages_prefix)
  (header_sep
    (colon)
    (tab)
  )
  (languages_contents
    (language_code)
  )
  (newline)
)
```

## Metadata

- **Level**: header
- **Category**: header
