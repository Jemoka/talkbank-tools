# E307: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E307
- **Category**: validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E3xx_main_tier_errors/E307_invalid_speaker_chars.cha`
**Trigger**: See example below
**Expected Error Codes**: E370

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	A:B Child
@ID:	eng|corpus|A:B|||||Child|||
*A:B:	hello .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
