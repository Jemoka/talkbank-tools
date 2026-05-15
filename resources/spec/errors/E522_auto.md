# E522: @Participants header cannot be empty

## Description

@Participants header cannot be empty

## Metadata

- **Error Code**: E522
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E506_empty_participants.cha`
**Trigger**: @Participants with empty content after colon-tab
**Expected Error Codes**: E342

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	

@End
```

## Example 2

**Source**: `E5xx_header_errors/E522_missing_id_for_participant.cha`
**Trigger**: CHI in @Participants but no @ID for CHI
**Expected Error Codes**: E522

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child, MOT Mother
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello .
*MOT:	hi there .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
