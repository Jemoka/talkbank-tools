# E529: Nested background with identical label

## Description

Nested background with identical label

## Metadata

- **Error Code**: E529
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/nested-bg-same-label.cha`
**Trigger**: See example below
**Expected Error Codes**: E529

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Bg:test
@Comment:	This is inside the first @Bg:test scope
@Bg:test
@Comment:	ERROR: This second @Bg:test should be invalid (nested @Bg with same label)
@Eg:test
@Eg:test
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
