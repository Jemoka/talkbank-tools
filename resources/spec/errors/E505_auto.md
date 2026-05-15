# E505: Invalid @ID format

## Description

Invalid @ID format

## Metadata

- **Error Code**: E505
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E505_invalid_id_format.cha`
**Trigger**: @ID with only 2 fields (needs 4+)
**Expected Error Codes**: E505

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus
@End
```

## Example 2

**Source**: `E5xx_header_errors/E511_empty_id_participant.cha`
**Trigger**: @ID with empty participant field (third field)
**Expected Error Codes**: E505

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus||
@End
```

## Example 3

**Source**: `E5xx_header_errors/E510_empty_id_language.cha`
**Trigger**: @ID with empty language field (first field)
**Expected Error Codes**: E505

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	|corpus|CHI|
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
