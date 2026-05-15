# E512: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E512
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E512_empty_participant_code.cha`
**Trigger**: See example below
**Expected Error Codes**: E513

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	Child
*CHI:	hello .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
