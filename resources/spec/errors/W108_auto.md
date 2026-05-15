# W108: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: W108
- **Category**: validation
- **Level**: file
- **Layer**: validation

## Example 1

**Source**: `error_corpus/warnings/W108_speaker_not_in_participants.cha`
**Trigger**: See example below
**Expected Error Codes**: W108

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*MOT:	hello .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See example above

## CHAT Rule

See the CHAT manual for format specifications: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
