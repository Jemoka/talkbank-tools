# E362: Bullet timestamps must be monotonic

## Description

Bullet timestamps must be monotonic

## Metadata

- **Error Code**: E362
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/bullet-timestamp-backwards.cha`
**Trigger**: See example below
**Expected Error Codes**: E362

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Media:	sample, audio
*CHI:	hello there . 10000_12000
@Comment:	First bullet: 10000-12000ms
*CHI:	how are you . 8000_9000
@Comment:	ERROR: Second bullet starts at 8000ms, which is BEFORE the first bullet
@Comment:	Timestamps must be monotonically increasing
*CHI:	I am fine . 15000_17000
@Comment:	VALID: 15000 > 12000
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
