# E509: @Media header cannot be empty

## Description

@Media header cannot be empty

## Metadata

- **Error Code**: E509
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E509_empty_media.cha`
**Trigger**: @Media with no content after colon-tab
**Expected Error Codes**: E509

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Media:
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
