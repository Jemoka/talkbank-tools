# E532: Invalid participant role

## Description

Invalid participant role

## Metadata

- **Error Code**: E532
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/invalid-participant-role.cha`
**Trigger**: See example below
**Expected Error Codes**: E532

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother, INV Investigator, BOB InvalidRole
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@ID:	eng|corpus|MOT|30;00.|female|||Mother|||
@ID:	eng|corpus|INV|25;00.|female|||Investigator|||
@ID:	eng|corpus|BOB|35;00.|male|||InvalidRole|||
@Comment:	ERROR: "InvalidRole" is not a valid participant role
@Comment:	Valid roles include: Target_Child, Mother, Father, Investigator, etc.
*CHI:	hello .
*MOT:	hi sweetie .
*BOB:	I have an invalid role .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
