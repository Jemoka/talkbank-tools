# id_trailing_whitespace

@ID header with trailing whitespace in the custom field (10th field).
Trailing whitespace must be trimmed — it is not semantically meaningful.

Found in: aphasia-data/English/Protocol/Fridriksson-2 (239+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```id_header
@ID:	eng|corpus|PAR|43;|male|Broca||Participant||73.9 |
```

## Expected CST

```cst
(id_header
  (id_prefix)
  (header_sep)
  (id_fields)
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
