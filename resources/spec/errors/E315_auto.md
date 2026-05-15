# E315: Invalid control character

## Description

Main tier or dependent tier contains an invalid control character (e.g., embedded NUL, SOH, or other non-printable ASCII).

## Metadata
- **Status**: implemented

- **Error Code**: E315
- **Category**: validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E3xx_main_tier_errors/E315_control_character.cha`
**Trigger**: See example below
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	wordtest .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
