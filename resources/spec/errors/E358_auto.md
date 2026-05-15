# E358: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E358
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E358_unmatched_longfeature_begin.cha`
**Trigger**: See example below
**Expected Error Codes**: E358

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	I am &{l=singing happy birthday to you .
*CHI:	another utterance .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
