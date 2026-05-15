# E231: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E231
- **Category**: validation
- **Level**: word
- **Layer**: parser

## Example 1

**Source**: `E2xx_word_errors/E231_unbalanced_shortening.cha`
**Trigger**: See example below
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hel(o .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
