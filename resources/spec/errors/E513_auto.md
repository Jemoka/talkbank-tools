# E513: Participant entry should have both code and role

## Description

Participant entry should have both code and role

## Metadata

- **Error Code**: E513
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E512_participant_no_role.cha`
**Trigger**: @Participants with only participant code, no role
**Expected Error Codes**: E513

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI
@ID:	eng|corpus|CHI|||||CHI|||
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
