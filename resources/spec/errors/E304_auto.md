# E304: Expected terminator not found

## Description

Expected terminator not found

## Metadata

- **Error Code**: E304
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E304_missing_terminator.cha`
**Trigger**: Utterance without terminator (., ?, or !)
**Expected Error Codes**: E304

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
