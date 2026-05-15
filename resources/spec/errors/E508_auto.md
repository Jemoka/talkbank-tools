# E508: @Date header cannot be empty

## Description

@Date header cannot be empty

## Metadata

- **Error Code**: E508
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E508_empty_date.cha`
**Trigger**: @Date with no content after colon-tab
**Expected Error Codes**: E516

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Date:
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
