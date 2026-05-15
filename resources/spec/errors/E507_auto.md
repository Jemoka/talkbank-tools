# E507: @Languages header cannot be empty

## Description

@Languages header cannot be empty

## Metadata

- **Error Code**: E507
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E507_empty_languages.cha`
**Trigger**: @Languages with no content after colon-tab
**Expected Error Codes**: E507

```chat
@UTF8
@Begin
@Languages:
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
