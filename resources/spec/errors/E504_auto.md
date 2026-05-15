# E504: Missing required header

## Description

Missing required header

## Metadata

- **Error Code**: E504
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E504_missing_languages.cha`
**Trigger**: Missing @Languages header
**Expected Error Codes**: E504

```chat
@UTF8
@Begin
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
```

## Example 2

**Source**: `E5xx_header_errors/E504_missing_participants.cha`
**Trigger**: Missing @Participants header
**Expected Error Codes**: E504

```chat
@UTF8
@Begin
@Languages:	eng
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
