# E527: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E527
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E527_unmatched_end_gem.cha`
**Trigger**: See example below
**Expected Error Codes**: E527

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	hello world .
@Eg:	episode1
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
