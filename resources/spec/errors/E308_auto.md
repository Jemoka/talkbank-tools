# E308: Invalid speaker format

## Description

Invalid speaker format

## Metadata

- **Error Code**: E308
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E302_invalid_speaker_format.cha`
**Trigger**: Speaker code with invalid characters
**Expected Error Codes**: E308

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CH-I:	hello world .
@End
```

## Example 2

**Source**: `E3xx_main_tier_errors/E308_speaker_not_in_participants.cha`
**Trigger**: Speaker code not listed in @Participants header
**Expected Error Codes**: E308

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*MOT:	hello world .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
