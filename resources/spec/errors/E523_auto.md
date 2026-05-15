# E523: Orphan @ID header

## Description

Orphan @ID header

## Metadata

- **Error Code**: E523
- **Category**: Participant validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E523_orphan_id_header.cha`
**Trigger**: @ID for MOT but MOT not in @Participants
**Expected Error Codes**: E523

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child
@ID:	eng|corpus|CHI|2;06.00||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
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
