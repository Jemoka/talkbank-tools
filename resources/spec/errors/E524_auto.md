# E524: @Birth header for unknown participant

## Description

@Birth header for unknown participant

## Metadata

- **Error Code**: E524
- **Category**: Participant validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E524_birth_unknown_participant.cha`
**Trigger**: @Birth of MOT but MOT not in @Participants
**Expected Error Codes**: E524

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child
@ID:	eng|corpus|CHI|2;06.00||||Target_Child|||
@Birth of MOT:	01-JAN-2000
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
