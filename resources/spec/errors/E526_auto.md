# E526: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E526
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E526_unmatched_begin_gem.cha`
**Trigger**: See example below
**Expected Error Codes**: E526

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
@Bg:	episode1
*CHI:	hello world .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
